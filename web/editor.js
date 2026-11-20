/** Metadata-only, non-destructive basic edits. Originals remain unchanged. */
export const DEFAULT_EDITS=Object.freeze({brightness:100,contrast:100,saturation:100,rotation:0});
const clamp=(n,min,max,def)=>Number.isFinite(Number(n))?Math.max(min,Math.min(max,Number(n))):def;
export function normalizedEdits(edits={}){
  return {
    brightness:clamp(edits.brightness,20,200,100),
    contrast:clamp(edits.contrast,20,200,100),
    saturation:clamp(edits.saturation,0,250,100),
    rotation:((Number(edits.rotation)||0)%360+360)%360,
  };
}
export function photoFilter(edits={}){
  const e=normalizedEdits(edits);
  return `brightness(${e.brightness}%) contrast(${e.contrast}%) saturate(${e.saturation}%)`;
}
export async function renderEditedBlob(original,edits={}){
  const e=normalizedEdits(edits),image=await createImageBitmap(original,{imageOrientation:'from-image'});
  try{
    const scale=Math.min(1,6000/Math.max(image.width,image.height));
    const w=Math.max(1,Math.round(image.width*scale)),h=Math.max(1,Math.round(image.height*scale));
    const swap=e.rotation===90||e.rotation===270;
    const canvas=document.createElement('canvas');canvas.width=swap?h:w;canvas.height=swap?w:h;
    const context=canvas.getContext('2d',{alpha:false});if(!context)throw new Error('Canvas output unavailable');
    context.fillStyle='#fff';context.fillRect(0,0,canvas.width,canvas.height);
    context.filter=photoFilter(e);context.translate(canvas.width/2,canvas.height/2);context.rotate(e.rotation*Math.PI/180);
    context.drawImage(image,-w/2,-h/2,w,h);
    const blob=await new Promise(resolve=>canvas.toBlob(resolve,'image/jpeg',.94));
    if(!blob)throw new Error('Could not export edited image');
    return blob;
  }finally{image.close?.();}
}
