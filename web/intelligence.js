/**
 * Mnemosyne's explainable, entirely offline rule engine.
 * This is NOT a visual AI or facial recognition model. A future ML adapter may
 * contribute model predictions, but all unverified identities remain in review.
 */
export const SORT_ENGINE_VERSION = 2;
const platforms = [
  ['Instagram', /(?:^|[\/\s._-])(?:instagram|insta|reels?)(?:[\/\s._-]|$)/i],
  ['TikTok', /(?:^|[\/\s._-])(?:tiktok|douyin)(?:[\/\s._-]|$)/i],
  ['YouTube', /(?:^|[\/\s._-])(?:youtube|ytshorts|youtube shorts)(?:[\/\s._-]|$)/i],
  ['Snapchat', /(?:^|[\/\s._-])(?:snapchat|spotlight|snap)(?:[\/\s._-]|$)/i],
  ['Facebook', /(?:^|[\/\s._-])(?:facebook|fb_reel)(?:[\/\s._-]|$)/i],
  ['WhatsApp', /(?:^|[\/\s._-])(?:whatsapp|wa_image|wa_video)(?:[\/\s._-]|$)/i],
  ['Telegram', /(?:^|[\/\s._-])(?:telegram|tg_video)(?:[\/\s._-]|$)/i],
  ['Signal', /(?:^|[\/\s._-])signal(?:[\/\s._-]|$)/i],
  ['Discord', /(?:^|[\/\s._-])discord(?:[\/\s._-]|$)/i],
  ['Reddit', /(?:^|[\/\s._-])reddit(?:[\/\s._-]|$)/i],
  ['Pinterest', /(?:^|[\/\s._-])(?:pinterest|idea pins)(?:[\/\s._-]|$)/i],
  ['X / Twitter', /(?:^|[\/\s._-])(?:twitter|tweet|x_twitter)(?:[\/\s._-]|$)/i],
  ['CapCut', /(?:^|[\/\s._-])capcut(?:[\/\s._-]|$)/i],
  ['Google Photos Export', /(?:google[ _-]?photos|takeout)[\/\\]/i],
  ['Apple Photos Export', /(?:apple[ _-]?photos|icloud[ _-]?photos)[\/\\]/i],
];
const subjects = {
  Travel: /(?:^|[^a-z])(beach|vacation|trip|travel|holiday|flight|passport|airport|journey)(?:$|[^a-z])/i,
  Nature: /(?:^|[^a-z])(flower|forest|sunset|sunrise|mountain|landscape|lake|waterfall|garden)(?:$|[^a-z])/i,
  Celebrations: /(?:^|[^a-z])(wedding|birthday|anniversary|graduation|celebration|party)(?:$|[^a-z])/i,
  Food: /(?:^|[^a-z])(dinner|breakfast|lunch|cooking|recipe|coffee|dessert)(?:$|[^a-z])/i,
  Pets: /(?:^|[^a-z])(dog|puppy|cat|kitten|rabbit|pet)(?:$|[^a-z])/i,
  Portraits: /(?:^|[^a-z])(portrait|selfie|headshot)(?:$|[^a-z])/i,
  Documents: /(?:^|[^a-z])(scan|receipt|invoice|document|whiteboard)(?:$|[^a-z])/i,
};
const stableTime = stamp => Number.isFinite(stamp) && stamp > 0 ? new Date(stamp).getFullYear() : null;
export function sourceOf(item){
  const path = `${item.folder || item.virtualPath || ''}/${item.name || ''}`;
  const hit = platforms.find(([,pattern]) => pattern.test(path));
  return hit ? hit[0] : item.source || 'This Device';
}
export function isShortVideo(item){
  if(item.kind !== 'video') return false;
  // Duration, not aspect ratio, determines a short: horizontal shorts count too.
  if(Number.isFinite(item.duration) && item.duration >= 0) return item.duration <= 180;
  return false;
}
export function proposeOrganization(item, { modelPredictions=[] } = {}){
  const automatic=[],review=[];
  const push=(target,label,reason,confidence,origin='rule')=>{
    if(!target.some(s=>s.label===label))target.push({label,reason,confidence,origin});
  };
  const source=sourceOf(item),year=stableTime(item.takenAt);
  if(year && year>=1970 && year<=2200) push(automatic,String(year),'Known file date',1);
  if(item.kind==='video')push(automatic,'Videos','Media type',1);
  if(item.kind==='image')push(automatic,'Photographs','Media type',1);
  if(source!=='This Device')push(automatic,source,'Recognized export or folder name',0.99);
  if(isShortVideo(item))push(automatic,'Shorts',`Video duration ${Math.round(item.duration)} seconds`,0.99);
  else if(item.kind==='video' && !Number.isFinite(item.duration) && /^(TikTok|Snapchat|CapCut)$/i.test(source))
    push(review,'Shorts','Likely short-form export; playback duration unknown',0.70);
  if(/(?:^|[\\/])screenshots?(?:[\\/]|$)|(?:^|[^a-z])screen(?:shot|_shot| shot)/i.test(`${item.folder || ''}/${item.name || ''}`))
    push(automatic,'Screenshots','Filename or screenshot directory',0.98);
  if(item.kind==='video'&&/(?:^|[^a-z])screen.?record/i.test(item.name||''))
    push(automatic,'Screen Recordings','Filename explicitly says screen recording',0.98);
  const haystack=`${item.folder||''} ${item.name||''}`;
  for(const [name,pattern] of Object.entries(subjects)){
    if(pattern.test(haystack))push(review,name,'Filename or folder keyword — not visual recognition',0.62);
  }
  // Model predictions are never treated as absolute evidence; only an audited
  // adapter can supply them, and the person name is never guessed from pixels.
  for(const suggestion of modelPredictions){
    if(!suggestion || typeof suggestion.label!=='string'||!Number.isFinite(suggestion.confidence))continue;
    const label=suggestion.label.trim().slice(0,80);
    if(!label)continue;
    push(review,label,'Unverified AI model suggestion',Math.max(0,Math.min(0.99,suggestion.confidence)),'model');
  }
  return {source,automatic,review,version:SORT_ENGINE_VERSION};
}
export function applyAutomatic(item, plan=proposeOrganization(item)){
  const tags=new Set(item.tags||[]);
  for(const prediction of plan.automatic)tags.add(prediction.label);
  return {...item,source:plan.source,tags:[...tags],reviewSuggestions:plan.review,sortEngineVersion:plan.version};
}
export function applyReviewLabels(item,labels){
  const chosen=new Set(labels);const tags=new Set(item.tags||[]);
  for(const suggestion of (item.reviewSuggestions||[]))if(chosen.has(suggestion.label))tags.add(suggestion.label);
  return {...item,tags:[...tags],reviewSuggestions:(item.reviewSuggestions||[]).filter(s=>!chosen.has(s.label))};
}
export function dismissReviewLabels(item,labels){const rejected=new Set(labels);return {...item,reviewSuggestions:(item.reviewSuggestions||[]).filter(s=>!rejected.has(s.label))};}
export function reviewSummary(items){let media=0,suggestions=0;for(const item of items){if(item.deletedAt)continue;const n=item.reviewSuggestions?.length||0;if(n){media++;suggestions+=n;}}return {media,suggestions};}
export function categorizeCatalog(items){const groups=new Map();for(const item of items){if(item.deletedAt)continue;for(const tag of item.tags||[]){groups.set(tag,(groups.get(tag)||0)+1);}}return [...groups].sort((a,b)=>b[1]-a[1]);}
