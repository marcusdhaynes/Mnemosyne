/** Google / Samsung Motion Photo detection. The still file is never rewritten:
 * we report where the embedded MP4 lives so it can be played or extracted
 * (`extractMotionVideo`) without loss.
 *
 * Order of evidence:
 * 1. XMP Motion Photo 1.0: `Camera:MotionPhoto=1` + GContainer `Container:Directory`
 *    item with `Item:Semantic="MotionPhoto"` and `Item:Length` (video is at the file end).
 * 2. Legacy Pixel XMP: `GCamera:MicroVideo=1` + `GCamera:MicroVideoOffset` (bytes from end).
 * 3. Samsung: `MotionPhoto_Data` marker followed by an MP4 (`ftyp`), before the SEF trailer.
 * Every candidate is validated by finding `ftyp` at the computed start.
 */
import {ascii,findInSource,indexOf} from './bytes.js';
import {mp4Extent} from './quicktime.js';
const MAX_SCAN_BYTES=200*1024*1024;
async function validAt(source,offset){if(!(offset>=0&&offset+12<=source.size))return false;return ascii(await source.read(offset,offset+8),4,8)==='ftyp';}
export async function detectMotionPhoto(source,xmp){
  const flagged=xmp&&(xmp.motionPhoto==='1'||xmp.microVideo==='1');
  if(xmp?.containerItems?.length){
    const items=xmp.containerItems;const at=items.findIndex(i=>/^video\//.test(i.mime)&&(i.semantic==='MotionPhoto'||flagged));
    if(at>0&&items[at].length>0){
      const tail=items.slice(at).reduce((n,i)=>n+i.length,0);
      const offset=source.size-tail;
      if(await validAt(source,offset))return {offset,length:items[at].length,mime:items[at].mime,source:'xmp-container',presentationTimestampUs:xmp.presentationTimestampUs?Number(xmp.presentationTimestampUs):null};
    }
  }
  if(xmp?.microVideo==='1'&&Number(xmp.microVideoOffset)>0){
    const offset=source.size-Number(xmp.microVideoOffset);
    if(await validAt(source,offset))return {offset,length:Number(xmp.microVideoOffset),mime:'video/mp4',source:'xmp-microvideo',presentationTimestampUs:null};
  }
  // Samsung: cheap tail check for the SEF trailer before any full scan.
  if(source.size>=8&&source.size<=MAX_SCAN_BYTES){
    const tailBytes=await source.read(source.size-4,source.size);
    if(ascii(tailBytes,0,4)==='SEFT'||flagged){
      const marker=await findInSource(source,'MotionPhoto_Data');
      if(marker>=0){
        const window=await source.read(marker,Math.min(source.size,marker+4096));
        const ftyp=indexOf(window,'ftyp');
        if(ftyp>=4){
          const offset=marker+ftyp-4;
          const sefh=await findInSource(source,'SEFH',{from:offset});
          const extent=await mp4Extent(source,offset,sefh>offset?sefh:source.size);
          if(extent)return {offset,length:extent.length,mime:'video/mp4',source:'samsung-sef',presentationTimestampUs:null};
        }
      }
    }
  }
  return null;
}
/** Return the embedded video of a Motion Photo as a Blob (zero-copy slice for File/Blob inputs). */
export async function extractMotionVideo(file,motionVideo){
  if(!motionVideo)return null;
  const {offset,length,mime='video/mp4'}=motionVideo;
  if(typeof file.slice==='function')return file.slice(offset,offset+length,mime);
  const bytes=file.bytes?file.bytes.subarray(offset,offset+length):await file.read(offset,offset+length);
  return new Blob([bytes],{type:mime});
}
