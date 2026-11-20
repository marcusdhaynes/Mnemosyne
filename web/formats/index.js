/** Mnemosyne media-format layer — public API. See docs/MEDIA-FORMATS.md.
 *
 *   const items = await resolveMedia(files, {indexed});
 *
 * Groups Live Photos, Motion Photos, bursts and Snapchat overlays into single
 * items, recovers original capture dates from embedded metadata, export sidecars
 * and filenames, and tags shorts/screen recordings/slo-mo/time-lapse only when
 * the evidence is there. Read-only: never modifies, moves or deletes a file.
 */
import {toSource,sniff,isVideoContainer,dirname,basename,stem,ext} from './bytes.js';
import {jpegMetadata,pngMetadata,webpMetadata,heifMetadata,parseXmp} from './exif.js';
import {videoMetadata} from './quicktime.js';
import {detectMotionPhoto,extractMotionVideo} from './motion.js';
import {parseExifDate,parseIsoDate,dateFromFilename,isTakeoutSidecar,takeoutMatcher,takeoutDate,parseMetaExport,parseSnapchatJson,parseSnapchatHtml,snapchatMatcher,chooseDate,confidenceOf,fixMojibake} from './dates.js';
export {extractMotionVideo,fixMojibake,dateFromFilename};
export const MEDIA_FORMATS_VERSION=1;
export const SHORT_MAX_SECONDS=180;
const IMAGE_EXT=new Set(['jpg','jpeg','heic','heif','png','webp','avif','gif','dng','tif','tiff']);
const VIDEO_EXT=new Set(['mov','mp4','m4v','3gp','webm','mkv']);
const SIDECAR_RE=/\.(json|html?)$/i;
const HEAD_BYTES=512*1024;
const LIVE_MAX_GAP_MS=5000,LIVE_MAX_VIDEO_SECONDS=6;
const BURST_MAX_GAP_MS=900,BURST_MIN_TIMED_FRAMES=3;

/** Per-file metadata needed by this layer (not a general EXIF dump; the native indexer owns that). */
export async function inspectFile(file){
  const src=toSource(file);
  const head=await src.read(0,Math.min(src.size,HEAD_BYTES));
  const container=sniff(head,src.name);
  const info={path:src.path,name:src.name,size:src.size,mtime:src.mtime,container,mediaKind:isVideoContainer(container)||VIDEO_EXT.has(ext(src.name))&&container==='unknown'?'video':'image',
    exif:null,xmp:null,contentIdentifier:null,burstUUID:null,motionVideo:null,video:null,embeddedDates:[],make:null,model:null,userComment:null};
  try{
    let meta=null;
    if(container==='jpeg')meta=jpegMetadata(head);
    else if(container==='png')meta=pngMetadata(head);
    else if(container==='webp')meta=webpMetadata(head);
    else if(container==='heic'||container==='avif')meta=await heifMetadata(src,head);
    if(meta){
      info.exif=meta.exif;info.xmp=parseXmp(meta.xmp);
      const e=meta.exif;
      if(e){
        info.make=e.make;info.model=e.model;info.userComment=e.userComment;
        info.contentIdentifier=e.apple?.contentIdentifier||null;info.burstUUID=e.apple?.burstUUID||null;
        const d=parseExifDate(e.dateTimeOriginal||e.dateTimeDigitized,e.offsetTimeOriginal,e.subSecOriginal);
        if(d)info.embeddedDates.push({source:'exif',...d});
      }
      if(info.xmp?.createDate){const d=parseIsoDate(info.xmp.createDate)||parseExifDate(info.xmp.createDate);if(d)info.embeddedDates.push({source:'xmp',...d});}
      info.motionVideo=await detectMotionPhoto(src,info.xmp);
    }else if(isVideoContainer(container)&&container!=='matroska'){
      const v=await videoMetadata(src);info.video=v;
      const k=v.keys||{};
      info.contentIdentifier=k['com.apple.quicktime.content.identifier']||null;
      info.make=k['com.apple.quicktime.make']||null;info.model=k['com.apple.quicktime.model']||null;
      const qt=parseIsoDate(k['com.apple.quicktime.creationdate']);
      if(qt)info.embeddedDates.push({source:'quicktime',...qt});
      if(v.createdUtc)info.embeddedDates.push({source:'quicktime-mvhd',takenAt:v.createdUtc,takenAtLocal:null,precision:'second'});
    }
  }catch(error){info.error=String(error?.message||error);}
  return info;
}

/** Evidence-based subtype tags for one inspected file. Never guesses beyond metadata/filenames. */
export function classifyFile(info,{filenameHit=null}={}){
  const tags=new Set(),evidence=[];
  const kindHint=filenameHit?.kind;
  if(kindHint==='screenshot'&&info.mediaKind==='image'){tags.add('screenshot');evidence.push(`filename:${filenameHit.rule}`);}
  if(info.mediaKind==='image'&&/^screenshot$/i.test(info.userComment||'')){tags.add('screenshot');evidence.push('exif:UserComment=Screenshot');}
  if(kindHint==='screen-recording'&&info.mediaKind==='video'){tags.add('screen-recording');evidence.push(`filename:${filenameHit.rule}`);}
  if(info.motionVideo){tags.add('motion-photo');evidence.push(`motion:${info.motionVideo.source}`);}
  const v=info.video;
  if(v){
    const capture=Number(info.video.keys?.['com.android.capture.fps']);
    if(Number.isFinite(capture)&&capture>0&&v.fps){
      if(capture>=100&&capture>v.fps*1.5){tags.add('slo-mo');evidence.push(`android-capture-fps:${capture}`);}
      else if(capture<v.fps/1.5){tags.add('time-lapse');evidence.push(`android-capture-fps:${capture}`);}
    }else if(v.fps>=100){tags.add('slo-mo');evidence.push(`frame-rate:${v.fps}`);}
    if(!tags.has('screen-recording')&&Number.isFinite(v.durationSec)&&v.durationSec>0&&v.durationSec<=SHORT_MAX_SECONDS){tags.add('short');evidence.push(`duration:${Math.round(v.durationSec*10)/10}s`);}
  }
  return {tags:[...tags],evidence};
}
const KIND_ORDER=['live-photo','motion-photo','burst','screen-recording','screenshot','slo-mo','time-lapse','short'];
const primaryKind=(tags,mediaKind)=>KIND_ORDER.find(k=>tags.includes(k))||(mediaKind==='video'?'video':'photo');

async function readJson(file){try{const t=typeof file.text==='function'?await file.text():new TextDecoder().decode(file.bytes||await file.read(0,file.size));return {text:t,json:/\.json$/i.test(file.name||file.path||'')?JSON.parse(t):null};}catch{return {text:null,json:null};}}
async function mapLimit(list,limit,fn){const out=new Array(list.length);let i=0;await Promise.all(Array.from({length:Math.min(limit,list.length)},async()=>{while(i<list.length){const n=i++;out[n]=await fn(list[n],n);}}));return out;}
const norm=p=>p.toLowerCase();

/**
 * Resolve a batch of files into logical media items.
 * @param {Array<File|Blob|{path,size,read?,bytes?,lastModified?}>} files media plus any export sidecars (.json/.html)
 * @param {{indexed?:Record<string,{takenAt?:number,gps?:{lat:number,lon:number}}>|Map, concurrency?:number}} [options]
 *   `indexed` = per-path values from the native indexer (top-priority date and GPS).
 * @returns {Promise<MediaItem[]>}
 */
export async function resolveMedia(files,{indexed=null,concurrency=8}={}){
  const indexLookup=path=>indexed?(indexed instanceof Map?indexed.get(path):indexed[path]):null;
  const sources=[...files].map(f=>({file:f,src:toSource(f)}));
  const sidecars=sources.filter(s=>SIDECAR_RE.test(s.src.name));
  const media=sources.filter(s=>!SIDECAR_RE.test(s.src.name)&&(IMAGE_EXT.has(ext(s.src.name))||VIDEO_EXT.has(ext(s.src.name))||/^(image|video)\//.test(s.file.type||'')));

  // ---- sidecars ----
  const takeoutByDir=new Map(),metaRecords=[],snapEntries=[],usedSidecars=new Set();
  for(const s of sidecars){
    const {text,json}=await readJson(s.file);if(text==null)continue;
    const dir=dirname(s.src.path);
    if(json&&isTakeoutSidecar(json)){(takeoutByDir.get(dir)||takeoutByDir.set(dir,[]).get(dir)).push({path:s.src.path,name:s.src.name,json});continue;}
    if(/memories_history\.json$/i.test(s.src.name)&&json){snapEntries.push(...parseSnapchatJson(json));usedSidecars.add(s.src.path);continue;}
    if(/memories_history\.html?$/i.test(s.src.name)){snapEntries.push(...parseSnapchatHtml(text));usedSidecars.add(s.src.path);continue;}
    if(json){const recs=parseMetaExport(json,{path:s.src.path});if(recs.length){metaRecords.push(...recs.map(r=>({...r,sidecar:s.src.path})));usedSidecars.add(s.src.path);}}
  }
  const takeoutMatchers=new Map([...takeoutByDir].map(([d,list])=>[d,takeoutMatcher(list)]));
  const metaByUri=new Map(),metaByName=new Map();
  for(const r of metaRecords){const u=norm(r.uri.replace(/^\.?\//,''));metaByUri.set(u,r);const b=basename(u);metaByName.set(b,metaByName.has(b)?null:r);}
  const snapMatch=snapEntries.length?snapchatMatcher(snapEntries):null;

  // ---- per-file inspection + dates ----
  const infos=await mapLimit(media,concurrency,async({file,src})=>{
    const info=await inspectFile(file);
    const hit=dateFromFilename(src.name);
    const candidates=[...info.embeddedDates];let gps=null,platform=hit?.platform||null;const files=[];
    const idx=indexLookup(src.path);
    if(idx&&Number.isFinite(idx.takenAt))candidates.push({source:'indexer',takenAt:idx.takenAt,takenAtLocal:null,precision:'second'});
    if(idx?.gps)gps={...idx.gps,source:'indexer'};
    const tm=takeoutMatchers.get(dirname(src.path))?.(src.name);
    if(tm){const d=takeoutDate(tm.json);usedSidecars.add(tm.path);files.push({path:tm.path,role:'sidecar'});platform??='takeout';if(d){candidates.push({source:d.source,takenAt:d.takenAt,takenAtLocal:null,precision:'second'});if(!gps&&d.gps)gps={...d.gps,source:'sidecar:takeout'};}}
    const lower=norm(src.path);
    const meta=[...metaByUri].find(([u])=>lower===u||lower.endsWith('/'+u))?.[1]||metaByName.get(basename(lower));
    if(meta){candidates.push({source:meta.source,takenAt:meta.takenAt,takenAtLocal:null,precision:'second'});files.push({path:meta.sidecar,role:'sidecar'});platform??=meta.source.includes('instagram')?'instagram':'facebook';}
    const snap=snapMatch?.(src.name,info.mediaKind==='video');
    if(snap){candidates.push({source:'sidecar:snapchat',takenAt:snap.takenAt,takenAtLocal:null,precision:'second'});platform??='snapchat';if(!gps&&snap.gps)gps={...snap.gps,source:'sidecar:snapchat'};}
    if(hit?.date)candidates.push({source:`filename:${hit.rule}`,...hit.date});
    if(Number.isFinite(src.mtime))candidates.push({source:'mtime',takenAt:src.mtime,takenAtLocal:null,precision:'second'});
    const chosen=chooseDate(candidates);
    const cls=classifyFile(info,{filenameHit:hit});
    return {info,src,filenameHit:hit,candidates,chosen,gps,platform,extraFiles:files,...cls};
  });

  // ---- grouping ----
  const consumed=new Set(),items=[];
  const byDirStem=new Map();
  for(const r of infos){const key=`${dirname(r.src.path)}|${stem(r.src.name).toLowerCase()}`;(byDirStem.get(key)||byDirStem.set(key,[]).get(key)).push(r);}
  const bestTime=r=>r.chosen?.takenAt??null;

  // Snapchat "-main" + "-overlay" stacks.
  const overlays=new Map();
  for(const r of infos){const m=/^(.*)-overlay\.(png|webp)$/i.exec(r.src.name);if(m)overlays.set(`${dirname(r.src.path)}|${m[1].toLowerCase()}`,r);}

  // Live Photos: ContentIdentifier first, then basename + close timestamps.
  const stills=infos.filter(r=>r.info.mediaKind==='image'&&['jpeg','heic'].includes(r.info.container));
  const movies=infos.filter(r=>r.info.mediaKind==='video');
  const movieById=new Map();for(const v of movies)if(v.info.contentIdentifier)movieById.set(v.info.contentIdentifier,v);
  const livePairs=[];
  for(const s of stills){
    if(s.info.contentIdentifier&&movieById.has(s.info.contentIdentifier)){const v=movieById.get(s.info.contentIdentifier);if(!consumed.has(v.src.path)){livePairs.push([s,v,'content-identifier']);consumed.add(s.src.path);consumed.add(v.src.path);}}
  }
  for(const s of stills){
    if(consumed.has(s.src.path)||s.info.motionVideo)continue;
    const baseStem=stem(s.src.name).replace(/ \(\d+\)$/,'').toLowerCase(),dir=dirname(s.src.path);
    const candidates=movies.filter(v=>!consumed.has(v.src.path)&&dirname(v.src.path)===dir&&stem(v.src.name).replace(/ \(\d+\)$/,'').toLowerCase()===baseStem);
    const v=candidates.find(v=>{
      if(s.info.contentIdentifier&&v.info.contentIdentifier&&s.info.contentIdentifier!==v.info.contentIdentifier)return false;
      const d=v.info.video?.durationSec;if(Number.isFinite(d)&&d>LIVE_MAX_VIDEO_SECONDS)return false;
      const a=bestTime(s),b=bestTime(v);return a!=null&&b!=null&&Math.abs(a-b)<=LIVE_MAX_GAP_MS;
    });
    if(v){livePairs.push([s,v,'basename-time']);consumed.add(s.src.path);consumed.add(v.src.path);}
  }
  for(const [s,v,how] of livePairs){
    items.push(buildItem({primary:s,kindTags:['live-photo',...s.tags.filter(t=>t!=='short')],pairedVideo:v,extra:{liveId:s.info.contentIdentifier||v.info.contentIdentifier||null,pairing:how},evidence:[`live:${how}`]}));
  }

  // Bursts: BurstUUID → filename patterns → tight timing on the same device.
  const burstGroups=new Map();
  const addBurst=(key,r,how,cover=false)=>{const g=burstGroups.get(key)||burstGroups.set(key,{key,how,frames:[]}).get(key);g.frames.push({r,cover});};
  const free=()=>infos.filter(r=>r.info.mediaKind==='image'&&!consumed.has(r.src.path)&&!r.info.motionVideo&&!r.tags.includes('screenshot'));
  for(const r of free())if(r.info.burstUUID)addBurst(`apple:${r.info.burstUUID}`,r,'burst-uuid');
  const samsung=new Map();
  for(const r of free()){
    if(r.info.burstUUID)continue;const n=r.src.name,dir=dirname(r.src.path);
    let m=/BURST(\d{14,17})/i.exec(n);if(m){addBurst(`file:${dir}|${m[1]}`,r,'filename',/COVER/i.test(n));continue;}
    m=/^(.*?)[._-]BURST[-_]?\d+/i.exec(n);if(m){addBurst(`file:${dir}|${m[1].toLowerCase()}`,r,'filename',/COVER/i.test(n));continue;}
    m=/^(\d{8}_\d{6})_(\d{3})\.(jpe?g|heic)$/i.exec(n);if(m){const k=`${dir}|${m[1]}`;(samsung.get(k)||samsung.set(k,[]).get(k)).push(r);}
  }
  for(const [k,list] of samsung)if(list.length>=2)for(const r of list)addBurst(`samsung:${k}`,r,'filename');
  const grouped=new Set([...burstGroups.values()].flatMap(g=>g.frames.map(f=>f.r.src.path)));
  const timed=free().filter(r=>!grouped.has(r.src.path)&&r.chosen?.source==='exif'&&r.chosen.precision==='ms'&&r.info.model)
    .sort((a,b)=>a.chosen.takenAt-b.chosen.takenAt);
  for(let i=0;i<timed.length;){
    let j=i+1;const same=(x,y)=>x.info.make===y.info.make&&x.info.model===y.info.model&&dirname(x.src.path)===dirname(y.src.path);
    while(j<timed.length&&same(timed[j-1],timed[j])&&timed[j].chosen.takenAt-timed[j-1].chosen.takenAt<=BURST_MAX_GAP_MS)j++;
    if(j-i>=BURST_MIN_TIMED_FRAMES)for(let k=i;k<j;k++)addBurst(`time:${timed[i].src.path}`,timed[k],'timing');
    i=j;
  }
  for(const g of burstGroups.values()){
    const frames=g.frames.sort((a,b)=>(bestTime(a.r)??0)-(bestTime(b.r)??0)||a.r.src.path.localeCompare(b.r.src.path));
    const cover=(frames.find(f=>f.cover)||frames[0]).r;
    for(const f of frames)consumed.add(f.r.src.path);
    items.push(buildItem({primary:cover,kindTags:['burst',...cover.tags],members:frames.map(f=>f.r),extra:{burstId:g.key,burstGrouping:g.how},evidence:[`burst:${g.how}`]}));
  }

  // Everything else (overlays attach to their -main file).
  const mainKeys=new Set(infos.map(r=>/^(.*)-main\.[^.]+$/i.exec(r.src.name)).map((m,i)=>m&&`${dirname(infos[i].src.path)}|${m[1].toLowerCase()}`).filter(Boolean));
  const overlayPaths=new Set([...overlays].filter(([k])=>mainKeys.has(k)).map(([,r])=>r.src.path));
  for(const r of infos){
    if(consumed.has(r.src.path)||overlayPaths.has(r.src.path))continue;
    const m=/^(.*)-main\.[^.]+$/i.exec(r.src.name);
    const overlay=m?overlays.get(`${dirname(r.src.path)}|${m[1].toLowerCase()}`)||null:null;
    items.push(buildItem({primary:r,kindTags:r.tags,overlay,evidence:[]}));
  }
  return items.sort((a,b)=>a.primaryPath.localeCompare(b.primaryPath));
}

function buildItem({primary,kindTags,pairedVideo=null,members=null,overlay=null,extra={},evidence}){
  const r=primary,tags=[...new Set(kindTags)];
  const date=r.chosen;
  const files=[{path:r.src.path,role:'primary'}];
  if(pairedVideo)files.push({path:pairedVideo.src.path,role:'live-video'});
  if(members)for(const m of members)if(m!==r)files.push({path:m.src.path,role:'burst-frame'});
  if(overlay)files.push({path:overlay.src.path,role:'overlay'});
  for(const f of [r,...(pairedVideo?[pairedVideo]:[]),...(members||[])])for(const x of f.extraFiles)if(!files.some(y=>y.path===x.path))files.push(x);
  const v=r.info.video||pairedVideo?.info.video||null;
  return {
    id:`${tags[0]||r.info.mediaKind}:${r.src.path}`,
    primaryPath:r.src.path,
    pairedVideoPath:pairedVideo?.src.path??null,
    overlayPath:overlay?.src.path??null,
    motionVideo:r.info.motionVideo,
    members:members?members.map(m=>m.src.path):null,
    coverPath:members?r.src.path:null,
    files,
    kind:primaryKind(tags,r.info.mediaKind),
    mediaKind:r.info.mediaKind,
    container:r.info.container,
    tags,
    burstId:extra.burstId??null,
    burstGrouping:extra.burstGrouping??null,
    liveId:extra.liveId??null,
    livePairing:extra.pairing??null,
    takenAt:date?.takenAt??null,
    takenAtLocal:date?.takenAtLocal??null,
    takenAtSource:date?.source??null,
    takenAtConfidence:confidenceOf(date?.source??null),
    takenAtPrecision:date?.precision??null,
    dateCandidates:r.candidates.map(c=>({source:c.source,takenAt:c.takenAt})),
    gps:r.gps,
    platform:r.platform,
    make:r.info.make,model:r.info.model,
    durationSec:v?.durationSec??null,width:v?.width||null,height:v?.height||null,fps:v?.fps??null,
    evidence:[...evidence,...r.evidence],
    error:r.info.error||null,
  };
}
