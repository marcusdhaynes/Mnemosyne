import {inspectFile,dateFromFilename} from './formats/index.js';
const MAX_IMAGE_EDGE=640;
const MAX_DIGEST_BYTES=80*1024*1024;
export async function digestFile(file){
  // WebCrypto has no universally supported incremental digest. Do not buffer
  // huge RAW files/movies into RAM for a convenience hash.
  if(file.size>MAX_DIGEST_BYTES)return null;
  const digest=await crypto.subtle.digest('SHA-256',await file.arrayBuffer());
  return [...new Uint8Array(digest)].map(v=>v.toString(16).padStart(2,'0')).join('');
}
function visualHash(source){
  const mini=document.createElement('canvas');mini.width=8;mini.height=8;
  const c=mini.getContext('2d',{willReadFrequently:true});if(!c)return null;
  c.drawImage(source,0,0,8,8);const pixels=c.getImageData(0,0,8,8).data;
  const luminosity=[];for(let i=0;i<pixels.length;i+=4)luminosity.push(pixels[i]*.299+pixels[i+1]*.587+pixels[i+2]*.114);
  const mean=luminosity.reduce((a,b)=>a+b,0)/luminosity.length;
  return luminosity.map(n=>n>=mean?'1':'0').join('');
}
async function toWebp(source,width,height){
  const scale=Math.min(1,MAX_IMAGE_EDGE/Math.max(width,height));
  const canvas=document.createElement('canvas');canvas.width=Math.max(1,Math.round(width*scale));canvas.height=Math.max(1,Math.round(height*scale));
  const ctx=canvas.getContext('2d',{alpha:false});if(!ctx) return null;
  ctx.drawImage(source,0,0,canvas.width,canvas.height);
  return new Promise(resolve=>canvas.toBlob(resolve,'image/webp',.74));
}
async function videoThumbnail(file){
  const url=URL.createObjectURL(file),video=document.createElement('video');
  video.preload='metadata';video.muted=true;video.playsInline=true;
  const wait=(event,timeout=6000)=>new Promise((resolve,reject)=>{
    const timer=setTimeout(()=>{clear();reject(new Error('Video metadata timed out'));},timeout);
    const ok=()=>{clear();resolve();},err=()=>{clear();reject(new Error('Video codec unavailable'));};
    const clear=()=>{clearTimeout(timer);video.removeEventListener(event,ok);video.removeEventListener('error',err);};
    video.addEventListener(event,ok,{once:true});video.addEventListener('error',err,{once:true});
  });
  try{
    video.src=url;await wait('loadedmetadata');
    const duration=Number.isFinite(video.duration)?video.duration:null;
    const width=video.videoWidth||null,height=video.videoHeight||null;
    let thumbnail=null,phash=null;
    if(width&&height){
      try{
        const frame=wait('seeked',3500);video.currentTime=Math.min(Math.max(0.05,(duration||0)*.03),Math.max(0.05,(duration||0)-.05));
        await frame;thumbnail=await toWebp(video,width,height);phash=visualHash(video);
      }catch{/* Some codecs expose metadata but cannot decode frames. */}
    }
    return {thumbnail,width,height,phash,duration};
  }catch{return {thumbnail:null,width:null,height:null,phash:null,duration:null};}
  finally{video.removeAttribute('src');video.load();URL.revokeObjectURL(url);}
}
/** Best capture timestamp for one file (ms). JPEG, HEIC, PNG, WebP EXIF and MOV/MP4
 * QuickTime dates via web/formats, then filename patterns, then mtime. For grouped
 * items and export sidecars use `resolveMedia` from web/formats/index.js instead. */
export async function capturedAt(file){
  try{
    const info=await inspectFile(file);
    const order=['exif','quicktime','xmp','quicktime-mvhd'];
    const best=[...info.embeddedDates].sort((a,b)=>order.indexOf(a.source)-order.indexOf(b.source))[0];
    if(best)return best.takenAt;
    const named=dateFromFilename(file.name||'');
    if(named?.date)return named.date.takenAt;
  }catch{/* A malformed metadata block must never prevent import. */}
  return file.lastModified||Date.now();
}
export async function createThumbnail(file){
  if(file.type.startsWith('video/')||/\.(mp4|mov|m4v|webm|3gp|ogv)$/i.test(file.name))return videoThumbnail(file);
  if(!file.type.startsWith('image/')&&!/\.(png|jpe?g|webp|gif|bmp|avif|heic|heif)$/i.test(file.name))return {thumbnail:null,width:null,height:null,phash:null,duration:null};
  let bitmap;
  try{bitmap=await createImageBitmap(file,{imageOrientation:'from-image'});}
  catch{return {thumbnail:null,width:null,height:null,phash:null,duration:null};}
  try{return {thumbnail:await toWebp(bitmap,bitmap.width,bitmap.height),width:bitmap.width,height:bitmap.height,phash:visualHash(bitmap),duration:null};}
  finally{bitmap.close?.();}
}
