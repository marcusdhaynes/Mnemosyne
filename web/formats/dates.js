/** Original-date recovery: filename patterns, Google Takeout, Instagram/Facebook
 * and Snapchat exports, plus the priority rule that picks a winner.
 * Pure functions; no I/O except parsing text the caller passes in.
 */
import {basename,dirname,stem} from './bytes.js';

/** Precedence (lower wins). `indexer` = value supplied by the native indexer. */
export const DATE_PRIORITY=['indexer','exif','quicktime','xmp','sidecar','quicktime-mvhd','filename','mtime'];
const MIN_TS=Date.UTC(1971,0,1);
const valid=(ms,now=Date.now())=>Number.isFinite(ms)&&ms>=MIN_TS&&ms<=now+86400000;
const pad=n=>String(n).padStart(2,'0');
function local(y,mo,d,h=0,mi=0,s=0,ms=0){
  y=+y;mo=+mo;d=+d;h=+h;mi=+mi;s=+s;
  if(mo<1||mo>12||d<1||d>31||h>23||mi>59||s>60)return null;
  const date=new Date(y,mo-1,d,h,mi,s,+ms||0);
  if(date.getMonth()!==mo-1)return null;
  return {takenAt:date.getTime(),takenAtLocal:`${y}-${pad(mo)}-${pad(d)}T${pad(h)}:${pad(mi)}:${pad(s)}`};
}
/** "YYYY:MM:DD HH:MM:SS" (+ optional "+02:00" offset and sub-seconds) → {takenAt,takenAtLocal,precision}. */
export function parseExifDate(value,offset=null,subsec=null){
  const m=/^(\d{4})[:-](\d{2})[:-](\d{2})[ T](\d{2}):(\d{2}):(\d{2})/.exec(value||'');if(!m||m[1]==='0000')return null;
  const ms=subsec&&/^\d+$/.test(subsec)?Math.round(Number(`0.${subsec}`)*1000):0;
  const base=local(m[1],m[2],m[3],m[4],m[5],m[6],ms);if(!base)return null;
  const o=/^([+-])(\d{2}):?(\d{2})$/.exec(offset||'');
  if(o){const sign=o[1]==='-'?-1:1;base.takenAt=Date.UTC(+m[1],+m[2]-1,+m[3],+m[4],+m[5],+m[6],ms)-sign*(+o[2]*60+ +o[3])*60000;base.offset=offset;}
  base.precision=subsec?'ms':'second';
  return valid(base.takenAt)?base:null;
}
/** ISO-8601 with optional offset, as used by com.apple.quicktime.creationdate. */
export function parseIsoDate(value){
  const m=/^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(Z|[+-]\d{2}:?\d{2})?/.exec(value||'');if(!m)return null;
  const offset=m[7]==='Z'?'+00:00':m[7]||null;
  return parseExifDate(`${m[1]}:${m[2]}:${m[3]} ${m[4]}:${m[5]}:${m[6]}`,offset);
}
const AMPM=(h,ap)=>ap?(+h%12)+(/p/i.test(ap)?12:0):+h;
/** Filename rules, most specific first. `kind` feeds classification; `precision` is honest about day-only dates. */
export const FILENAME_RULES=[
  {id:'whatsapp',re:/^(?:IMG|VID|AUD|PTT|STK|DOC)-(\d{4})(\d{2})(\d{2})-WA\d+/i,date:m=>local(m[1],m[2],m[3]),precision:'day',platform:'whatsapp'},
  {id:'screenshot-android',re:/^Screenshot[_-](\d{4})-?(\d{2})-?(\d{2})[-_](\d{2})-?(\d{2})-?(\d{2})/i,date:m=>local(m[1],m[2],m[3],m[4],m[5],m[6]),kind:'screenshot'},
  {id:'screenshot-macos',re:/^(?:Screenshot|Screen Shot) (\d{4})-(\d{2})-(\d{2}) at (\d{1,2})\.(\d{2})\.(\d{2})(?:[\s\u202f]*([AP]M))?/i,date:m=>local(m[1],m[2],m[3],AMPM(m[4],m[7]),m[5],m[6]),kind:'screenshot'},
  {id:'screenshot-windows',re:/^Screenshot (\d{4})-(\d{2})-(\d{2}) (\d{2})(\d{2})(\d{2})/i,date:m=>local(m[1],m[2],m[3],m[4],m[5],m[6]),kind:'screenshot'},
  {id:'screenshot',re:/^(?:Screenshot|Screen Shot|Capture d.écran|Bildschirmfoto)\b/i,date:()=>null,kind:'screenshot'},
  {id:'screen-recording-android',re:/^Screen[_ ]?Recording[_ -](\d{4})(\d{2})(\d{2})[-_](\d{2})(\d{2})(\d{2})/i,date:m=>local(m[1],m[2],m[3],m[4],m[5],m[6]),kind:'screen-recording'},
  {id:'screen-recording-macos',re:/^Screen Recording (\d{4})-(\d{2})-(\d{2}) at (\d{1,2})\.(\d{2})\.(\d{2})(?:[\s\u202f]*([AP]M))?/i,date:m=>local(m[1],m[2],m[3],AMPM(m[4],m[7]),m[5],m[6]),kind:'screen-recording'},
  // iOS ReplayKit exports. The digits look like Unix seconds; used only when plausible.
  {id:'screen-recording-ios',re:/^RPReplay_Final(\d{9,10})\b/i,date:m=>{const t=+m[1]*1000;return valid(t)&&t>=Date.UTC(2017,0,1)?{takenAt:t,takenAtLocal:null}:null;},kind:'screen-recording'},
  {id:'camera',re:/^(?:PXL|IMG|VID|MVIMG|PANO|BURST|LRV|Photo|Video)?[_-]?(\d{4})(\d{2})(\d{2})[_-](\d{2})(\d{2})(\d{2})(\d{3})?/i,date:m=>local(m[1],m[2],m[3],m[4],m[5],m[6],m[7]||0)},
  {id:'iso',re:/^(\d{4})-(\d{2})-(\d{2})[_ T](\d{2})[-.:](\d{2})[-.:](\d{2})/,date:m=>local(m[1],m[2],m[3],m[4],m[5],m[6])},
];
/** Match a filename against FILENAME_RULES. Returns {rule,kind,platform,date|null}. */
export function dateFromFilename(name){
  for(const rule of FILENAME_RULES){
    const m=rule.re.exec(name);if(!m)continue;
    const date=rule.date(m);
    if(date&&!valid(date.takenAt))return {rule:rule.id,kind:rule.kind||null,platform:rule.platform||null,date:null};
    return {rule:rule.id,kind:rule.kind||null,platform:rule.platform||null,date:date?{...date,precision:rule.precision||'second'}:null};
  }
  return null;
}

/** Meta (Facebook/Instagram) exports store UTF-8 bytes as \u00XX code points. */
export function fixMojibake(value){
  if(typeof value!=='string'||!/[\u00c2-\u00f4][\u0080-\u00bf]/.test(value)||/[^\u0000-\u00ff]/.test(value))return value;
  try{return new TextDecoder('utf-8',{fatal:true}).decode(Uint8Array.from(value,c=>c.charCodeAt(0)));}catch{return value;}
}
function deepFix(value){
  if(typeof value==='string')return fixMojibake(value);
  if(Array.isArray(value))return value.map(deepFix);
  if(value&&typeof value==='object')return Object.fromEntries(Object.entries(value).map(([k,v])=>[k,deepFix(v)]));
  return value;
}

// ---------- Google Takeout ----------
const COUNTER=/\((\d+)\)$/;
export function isTakeoutSidecar(json){return !!json&&typeof json==='object'&&!Array.isArray(json)&&(json.photoTakenTime||json.creationTime)&&('title' in json);}
/** Index Takeout sidecars of one folder: [{path,name,json}] → matcher(mediaName). */
export function takeoutMatcher(sidecars){
  const entries=sidecars.map(s=>{
    let key=s.name.replace(/\.json$/i,'');let counter=0;
    const c=COUNTER.exec(key);if(c){counter=+c[1];key=key.slice(0,c.index);}
    return {...s,key,counter,title:typeof s.json.title==='string'?s.json.title:null,truncated:s.name.length>=46};
  });
  return mediaName=>{
    let base=mediaName,counter=0;
    const m=/^(.*)\((\d+)\)(\.[^.]+)$/.exec(mediaName);if(m){base=m[1]+m[3];counter=+m[2];}
    const variants=[base,base.replace(/-(edited|bearbeitet|modifié|editado)(\.[^.]+)$/i,'$2')];
    for(const b of variants){
      const sameCounter=entries.filter(e=>e.counter===counter);
      // 1) the sidecar's own title (most reliable), 2) exact names, 3) 46–51 char truncation.
      const hit=sameCounter.find(e=>e.title===b&&(e.key===b||e.key.startsWith(b)||b.startsWith(e.key)||`${b}.supplemental-metadata`.startsWith(e.key)))
        ||sameCounter.find(e=>e.key===b||e.key===`${b}.supplemental-metadata`||e.key===stem(b))
        ||sameCounter.find(e=>e.truncated&&e.key.length>=8&&`${b}.supplemental-metadata`.startsWith(e.key));
      if(hit)return hit;
    }
    return null;
  };
}
export function takeoutDate(json){
  const ts=Number(json?.photoTakenTime?.timestamp);
  const geo=json?.geoDataExif&&(json.geoDataExif.latitude||json.geoDataExif.longitude)?json.geoDataExif:json?.geoData&&(json.geoData.latitude||json.geoData.longitude)?json.geoData:null;
  return valid(ts*1000)?{takenAt:ts*1000,source:'sidecar:takeout',gps:geo?{lat:geo.latitude,lon:geo.longitude,alt:geo.altitude??null}:null}:null;
}

// ---------- Instagram / Facebook ----------
/** Walk any Meta export JSON and return [{uri,takenAt,source,title}] for each media record. */
export function parseMetaExport(json,{path=''}={}){
  const fixed=deepFix(json);const out=[];
  const platform=/instagram/i.test(path)||JSON.stringify(json).slice(0,4000).match(/"ig_|instagram/i)?'instagram':'facebook';
  const visit=(node,inherited)=>{
    if(Array.isArray(node)){for(const n of node)visit(n,inherited);return;}
    if(!node||typeof node!=='object')return;
    const created=Number(node.creation_timestamp??node.timestamp)||inherited;
    if(typeof node.uri==='string'&&/\.(jpe?g|png|heic|webp|gif|mp4|mov|m4v|webm)$/i.test(node.uri)){
      const exif=node.media_metadata?.photo_metadata?.exif_data||node.media_metadata?.video_metadata?.exif_data||[];
      const taken=Number((Array.isArray(exif)?exif:[exif]).find(e=>e&&e.taken_timestamp)?.taken_timestamp);
      if(valid(taken*1000))out.push({uri:node.uri,takenAt:taken*1000,source:`sidecar:${platform}-taken`,title:node.title||null});
      else if(valid(created*1000))out.push({uri:node.uri,takenAt:created*1000,source:`sidecar:${platform}`,title:node.title||null,note:'upload time, not capture time'});
    }
    for(const v of Object.values(node))if(v&&typeof v==='object')visit(v,created);
  };
  visit(fixed,null);
  return out;
}

// ---------- Snapchat Memories ----------
function snapDate(s){const m=/(\d{4})-(\d{2})-(\d{2}) (\d{2}):(\d{2}):(\d{2})\s*UTC/.exec(s||'');return m?Date.UTC(+m[1],+m[2]-1,+m[3],+m[4],+m[5],+m[6]):null;}
function snapIds(url){const ids=[];for(const k of ['mid','sid','uid']){const m=new RegExp(`[?&]${k}=([^&'"\\s]+)`).exec(url||'');if(m)ids.push(decodeURIComponent(m[1]).toLowerCase());}return ids;}
function snapGps(s){const m=/(-?\d+(?:\.\d+)?)\s*,\s*(-?\d+(?:\.\d+)?)/.exec(s||'');return m&&(+m[1]||+m[2])?{lat:+m[1],lon:+m[2]}:null;}
/** memories_history.json → [{takenAt,mediaType,ids,gps}] */
export function parseSnapchatJson(json){
  const list=json?.['Saved Media']||json?.saved_media||[];
  return list.map(e=>({takenAt:snapDate(e.Date),mediaType:e['Media Type']||null,ids:snapIds(e['Download Link']||e['Media Download Url']||''),gps:snapGps(e.Location)})).filter(e=>valid(e.takenAt));
}
/** memories_history.html table rows → same shape. */
export function parseSnapchatHtml(html){
  const out=[];
  for(const row of String(html).split(/<tr\b/i).slice(1)){
    const cells=[...row.matchAll(/<td[^>]*>([\s\S]*?)<\/td>/gi)].map(c=>c[1]);
    const takenAt=snapDate(cells.map(c=>c.replace(/<[^>]+>/g,'')).find(c=>/UTC/.test(c)));
    if(!valid(takenAt))continue;
    out.push({takenAt,mediaType:(cells[1]||'').replace(/<[^>]+>/g,'').trim()||null,ids:snapIds(row),gps:snapGps(cells.map(c=>c.replace(/<[^>]+>/g,'')).find(c=>/Latitude/i.test(c)))});
  }
  return out;
}
/** Match a media filename to a Snapchat entry: by mid/sid in the name, then by exact UTC timestamp prefix,
 * then by a unique entry on the same UTC day with the same media type. */
export function snapchatMatcher(entries){
  const byId=new Map();for(const e of entries)for(const id of e.ids)byId.set(id,e);
  return (name,isVideo)=>{
    const lower=name.toLowerCase();
    for(const [id,e] of byId)if(id.length>=8&&lower.includes(id))return e;
    const full=/^(\d{4})-(\d{2})-(\d{2})[_ ](\d{2})[-:](\d{2})[-:](\d{2})/.exec(name);
    if(full){const t=Date.UTC(+full[1],+full[2]-1,+full[3],+full[4],+full[5],+full[6]);const hit=entries.find(e=>e.takenAt===t);if(hit)return hit;}
    const day=/^(\d{4})-(\d{2})-(\d{2})/.exec(name);
    if(day){const start=Date.UTC(+day[1],+day[2]-1,+day[3]);const type=isVideo?/video/i:/image|photo/i;
      const same=entries.filter(e=>e.takenAt>=start&&e.takenAt<start+86400000&&(!e.mediaType||type.test(e.mediaType)));
      if(same.length===1)return same[0];}
    return null;
  };
}

/** Pick the winning date. `candidates`: [{source,takenAt,...}]. Social sidecars beat the mvhd
 * box because platforms re-encode video and reset it to processing time. */
export function chooseDate(candidates){
  const rank=c=>{const tier=c.source.startsWith('sidecar:')?'sidecar':c.source.startsWith('filename:')?'filename':c.source;return DATE_PRIORITY.indexOf(tier);};
  const usable=candidates.filter(c=>c&&valid(c.takenAt));
  usable.sort((a,b)=>rank(a)-rank(b));
  return usable[0]||null;
}
export const confidenceOf=source=>source==null?'none':source==='indexer'||['exif','quicktime','xmp','quicktime-mvhd'].includes(source)?'embedded':source.startsWith('sidecar:')?'sidecar':source.startsWith('filename:')?'filename':'filesystem';
export {basename,dirname};
