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
/** Read JPEG EXIF capture timestamp without decoding the full image. */
export async function capturedAt(file){
  if(!/\.jpe?g$/i.test(file.name))return file.lastModified||Date.now();
  try{
    const bytes=new Uint8Array(await file.slice(0,262144).arrayBuffer());
    if(bytes[0]!==0xff||bytes[1]!==0xd8)return file.lastModified||Date.now();
    for(let offset=2;offset+12<bytes.length;){
      if(bytes[offset]!==0xff)break;
      const marker=bytes[offset+1],size=bytes[offset+2]*256+bytes[offset+3];
      if(size<2||offset+2+size>bytes.length)break;
      const start=offset+4;
      if(marker===0xe1&&String.fromCharCode(...bytes.subarray(start,start+4))==='Exif'){
        const base=start+6,little=bytes[base]===73&&bytes[base+1]===73;
        if(!little&&!(bytes[base]===77&&bytes[base+1]===77))break;
        const view=new DataView(bytes.buffer);
        const u16=p=>p+2<=bytes.length?view.getUint16(p,little):0;
        const u32=p=>p+4<=bytes.length?view.getUint32(p,little):0;
        const ifd=relative=>{const pos=base+relative,count=u16(pos);const records=[];for(let i=0;i<Math.min(count,100);i++){let at=pos+2+i*12;if(at+12>bytes.length)break;records.push({tag:u16(at),type:u16(at+2),length:u32(at+4),value:u32(at+8)});}return records;};
        const primary=ifd(u32(base+4)),exif=primary.find(r=>r.tag===0x8769);
        if(exif){const dateTag=ifd(exif.value).find(r=>r.tag===0x9003||r.tag===0x9004);if(dateTag&&dateTag.length>=19){
          const pos=base+dateTag.value;
          if(pos+19<=bytes.length){const raw=new TextDecoder().decode(bytes.subarray(pos,pos+19));
            if(/^\d{4}:\d{2}:\d{2} \d{2}:\d{2}:\d{2}$/.test(raw)){
              const dt=new Date(raw.replace(/^(\d{4}):(\d{2}):(\d{2}) /,'$1-$2-$3T'));
              if(Number.isFinite(dt.getTime()))return dt.getTime();
            }
          }
        }}
        break;
      }
      offset+=2+size;
    }
  }catch{/* A malformed EXIF segment must never prevent import. */}
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
