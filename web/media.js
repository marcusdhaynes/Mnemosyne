export async function digestFile(file) {
  // Avoid loading huge movies / RAW files into RAM just to fingerprint them.
  if(file.size>80*1024*1024)return null;
  const digest=await crypto.subtle.digest('SHA-256',await file.arrayBuffer());
  return [...new Uint8Array(digest)].map(v=>v.toString(16).padStart(2,'0')).join('');
}
export async function createThumbnail(file){
  if(!file.type.startsWith('image/') && !/\.(png|jpe?g|webp|gif|bmp|avif)$/i.test(file.name))return {thumbnail:null,width:null,height:null,phash:null};
  let bitmap;
  try { bitmap=await createImageBitmap(file,{imageOrientation:'from-image'}); }
  catch {return {thumbnail:null,width:null,height:null,phash:null};}
  try {
    const {width,height}=bitmap;const scale=Math.min(1,720/Math.max(width,height));
    const canvas=document.createElement('canvas');canvas.width=Math.max(1,Math.floor(width*scale));canvas.height=Math.max(1,Math.floor(height*scale));
    const ctx=canvas.getContext('2d',{alpha:false,willReadFrequently:true});ctx.drawImage(bitmap,0,0,canvas.width,canvas.height);
    const mini=document.createElement('canvas');mini.width=8;mini.height=8;const m=mini.getContext('2d',{willReadFrequently:true});m.drawImage(bitmap,0,0,8,8);
    const pixels=m.getImageData(0,0,8,8).data;let values=[];for(let i=0;i<pixels.length;i+=4)values.push(pixels[i]*.299+pixels[i+1]*.587+pixels[i+2]*.114);
    const avg=values.reduce((a,b)=>a+b,0)/values.length;const phash=values.map(v=>v>=avg?'1':'0').join('');
    const thumbnail=await new Promise(resolve=>canvas.toBlob(resolve,'image/webp',.79));
    return {thumbnail,width,height,phash};
  } finally {bitmap.close?.();}
}
