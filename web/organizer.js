export const SUPPORTED_IMAGE_EXTENSIONS = new Set(['jpg','jpeg','png','webp','gif','bmp','tif','tiff','avif','heic','heif','jxl','dng','nef','cr2','arw','raf','orf','rw2']);
export const SUPPORTED_VIDEO_EXTENSIONS = new Set(['mp4','mov','m4v','webm','avi','mkv','3gp']);
export function extension(name) { return (name.split('.').pop() || '').toLowerCase(); }
export function mediaKind(file) {
  const ext = extension(file.name || '');
  if (file.type?.startsWith('image/') || SUPPORTED_IMAGE_EXTENSIONS.has(ext)) return 'image';
  if (file.type?.startsWith('video/') || SUPPORTED_VIDEO_EXTENSIONS.has(ext)) return 'video';
  return null;
}
export function formattedDate(timestamp, locale = 'en-US') {
  return new Intl.DateTimeFormat(locale, { month:'long', day:'numeric', year:'numeric' }).format(new Date(timestamp));
}
export function monthKey(timestamp) { const d=new Date(timestamp); return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}`; }
export function monthLabel(key) { return new Intl.DateTimeFormat('en-US',{month:'long',year:'numeric'}).format(new Date(`${key}-15T12:00:00`)); }
export function formatBytes(bytes) { if(bytes < 1024) return `${bytes} B`; const units=['KB','MB','GB','TB'];let i=-1;do {bytes/=1024;i++;} while(bytes>=1024 && i<units.length-1);return `${bytes.toFixed(bytes<10?1:0)} ${units[i]}`; }
export function suggestTags(filename) {
  const base=filename.replace(/\.[^.]+$/,'').toLowerCase();
  const categories={Travel:['beach','vacation','flight','trip','travel','journey','city','hotel','mountain'],Nature:['flower','forest','sunset','sunrise','sea','lake','tree','wildlife','landscape'],Celebrations:['wedding','birthday','anniversary','party','graduation'],Food:['dinner','breakfast','lunch','cake','coffee','recipe'],Pets:['dog','cat','puppy','kitten','pet'],Portraits:['portrait','selfie','headshot','family','friends']};
  return Object.entries(categories).filter(([,words])=>words.some(w=>new RegExp(`(^|[^a-z])${w}([^a-z]|$)`).test(base))).map(([k])=>k);
}
export function similarityScore(hashA,hashB) { if(!hashA||!hashB||hashA.length!==hashB.length)return Infinity;return [...hashA].reduce((n,c,i)=>n+(c!==hashB[i]),0); }
export function duplicateSets(items) {
  const byHash=new Map();for(const item of items){if(item.deletedAt||!item.hash)continue;const list=byHash.get(item.hash)||[];list.push(item);byHash.set(item.hash,list);}
  return [...byHash.values()].filter(v=>v.length>1);
}
export function visibleItems(items,{view='library',albumId=null,query='',type='all',month='all',sort='newest'}={}){
  let result=items.filter(item=>{
    if(view==='trash')return !!item.deletedAt;
    if(item.deletedAt)return false;
    if(view==='favorites'&&!item.favorite)return false;
    if(view==='duplicates'&&!items.some(other=>other.id!==item.id&&!other.deletedAt&&item.hash&&item.hash===other.hash))return false;
    if(view==='portraits'&&!(item.tags||[]).includes('Portraits'))return false;
    if(view==='album'&&!(item.albumIds||[]).includes(albumId))return false;
    if(type!=='all'&&item.kind!==type)return false;
    if(month!=='all'&&monthKey(item.takenAt)!==month)return false;
    const haystack=[item.name,item.folder,...(item.tags||[]),...(item.people||[])].join(' ').toLowerCase();
    return !query||haystack.includes(query.toLowerCase().trim());
  });
  return result.sort((a,b)=>sort==='oldest'?(a.takenAt-b.takenAt):(sort==='name'?a.name.localeCompare(b.name):b.takenAt-a.takenAt));
}
export function groupedByMonth(items) { return Object.groupBy ? Object.groupBy(items,item=>monthKey(item.takenAt)) : items.reduce((a,i)=>((a[monthKey(i.takenAt)]??=[]).push(i),a),{}); }
