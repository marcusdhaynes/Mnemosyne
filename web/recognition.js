/** Privacy-first optional face-detection adapter.
 * OS/browser FaceDetector may be missing; this detects boxes, never identities.
 * An audited, opt-in ONNX recognition engine can provide embeddings later.
 */
export function faceDetectorAvailable(host=globalThis){return typeof host.FaceDetector==='function';}
export async function detectLocalFaces(file,host=globalThis){
  if(!faceDetectorAvailable(host))return {status:'unavailable',faces:[],engine:'none'};
  if(typeof host.createImageBitmap!=='function')return {status:'unavailable',faces:[],engine:'none'};
  const bitmap=await host.createImageBitmap(file);
  try{
    const detector=new host.FaceDetector({fastMode:false,maxDetectedFaces:40});
    const detected=await detector.detect(bitmap);
    const faces=(detected||[]).slice(0,40).map(face=>({
      x:Math.max(0,face.boundingBox?.x||0),
      y:Math.max(0,face.boundingBox?.y||0),
      width:Math.max(0,face.boundingBox?.width||0),
      height:Math.max(0,face.boundingBox?.height||0),
    })).filter(f=>f.width>0&&f.height>0);
    return {status:'detected',engine:'device-face-detector',width:bitmap.width,height:bitmap.height,faces};
  }catch(error){return {status:'error',engine:'device-face-detector',faces:[],message:String(error?.message||error)};}
  finally{bitmap.close?.();}
}
/** No biometric labels should be inserted automatically from detections alone. */
export function makeFaceReview(photo,faces){
  if(!faces.length)return photo;
  const suggestions=photo.reviewSuggestions||[];
  if(!suggestions.some(x=>x.label==='Portraits'))suggestions.push({label:'Portraits',reason:`${faces.length} possible face${faces.length===1?'':'s'} detected; confirm before organizing`,confidence:0.80,origin:'device-detector'});
  return {...photo,faceCount:faces.length,reviewSuggestions:suggestions};
}
