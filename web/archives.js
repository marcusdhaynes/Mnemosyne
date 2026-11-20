/** ZIP import of user-exported photos and social clips. No network or dependency.
 * Strictly rejects path traversal, encryption, zip64 and suspicious sizes.
 * Browsers without the deflate-raw decompressor can import extracted folders.
 */
import {mediaKind} from './organizer.js';
const decoder = new TextDecoder('utf-8');
const MAX_ARCHIVE_BYTES = 1_500_000_000;
const MAX_ENTRIES = 60000;
const MAX_SINGLE_FILE = 950_000_000;
const MAX_TOTAL_EXTRACTED = 3_000_000_000;
function legalPath(path){return path&&!path.startsWith('/')&&!path.startsWith('\\')&&!/^[A-Za-z]:/.test(path)&&!path.includes('\0')&&!path.split(/[\\/]/).includes('..');}
function mime(path){const ext=path.split('.').pop()?.toLowerCase();return ({jpg:'image/jpeg',jpeg:'image/jpeg',png:'image/png',webp:'image/webp',gif:'image/gif',avif:'image/avif',heic:'image/heic',heif:'image/heif',mp4:'video/mp4',mov:'video/quicktime',m4v:'video/mp4',webm:'video/webm',mkv:'video/x-matroska',avi:'video/x-msvideo', '3gp':'video/3gpp'})[ext]||'';}
export async function expandZip(file,{onProgress=()=>{},maxFiles=MAX_ENTRIES}={}){
  if(!file || file.size>MAX_ARCHIVE_BYTES)throw new Error('Archive too large. Extract it in your file manager and import the folder instead.');
  const buffer=await file.arrayBuffer();const bytes=new Uint8Array(buffer);const view=new DataView(buffer);
  const u16=p=>view.getUint16(p,true),u32=p=>view.getUint32(p,true);
  let end=-1;
  for(let p=bytes.length-22;p>=Math.max(0,bytes.length-65557);p--){if(u32(p)===0x06054b50 && p+22+u16(p+20)===bytes.length){end=p;break;}}
  if(end<0)throw new Error('ZIP directory not found. Extract this archive manually.');
  const entries=u16(end+10),start=u32(end+16),length=u32(end+12);
  if(entries===65535||start===0xffffffff||length===0xffffffff)throw new Error('ZIP64 requires desktop extraction; import the extracted folder.');
  if(entries>Math.min(MAX_ENTRIES,maxFiles)||start+length>bytes.length)throw new Error('ZIP entries exceed safety limits.');
  const output=[];let offset=start,total=0,skipped=0;
  for(let i=0;i<entries;i++){
    if(offset+46>bytes.length||u32(offset)!==0x02014b50)throw new Error('Malformed ZIP directory.');
    const flag=u16(offset+8),method=u16(offset+10),compressed=u32(offset+20),unpacked=u32(offset+24),nameLen=u16(offset+28),extraLen=u16(offset+30),commentLen=u16(offset+32),local=u32(offset+42);
    const path=decoder.decode(bytes.subarray(offset+46,offset+46+nameLen)).replaceAll('\\','/');
    offset+=46+nameLen+extraLen+commentLen;
    if(!legalPath(path))throw new Error('Unsafe path in archive. Import canceled.');
    if(path.endsWith('/')||!mediaKind({name:path,type:mime(path)})){skipped++;continue;}
    if(flag&1||!(method===0||method===8)){skipped++;continue;}
    total+=unpacked;
    if(unpacked>MAX_SINGLE_FILE||total>MAX_TOTAL_EXTRACTED||unpacked>Math.max(1,compressed)*500)throw new Error('Suspiciously large compressed media. Extract outside Mnemosyne.');
    if(local+30>bytes.length||u32(local)!==0x04034b50)throw new Error('Malformed ZIP file header.');
    const body=local+30+u16(local+26)+u16(local+28);
    if(body+compressed>bytes.length)throw new Error('Truncated ZIP entry.');
    const segment=bytes.subarray(body,body+compressed);
    let blob;
    if(method===0)blob=new Blob([segment],{type:mime(path)});
    else{
      if(typeof DecompressionStream==='undefined')throw new Error('This browser cannot unpack ZIP files. Extract the folder and import it instead.');
      try{blob=await new Response(new Blob([segment]).stream().pipeThrough(new DecompressionStream('deflate-raw'))).blob();}
      catch{throw new Error('This browser cannot decode this ZIP compression. Extract the folder and import it instead.');}
    }
    if(blob.size!==unpacked)throw new Error(`Size mismatch in ${path}. Archive may be corrupt.`);
    const originalName=path.split('/').at(-1);
    const imported=new File([blob],originalName,{type:mime(path),lastModified:file.lastModified||Date.now()});
    Object.defineProperty(imported,'virtualPath',{value:path,enumerable:true});
    output.push(imported);
    if(i%16===0)onProgress(i+1,entries);
  }
  return {files:output,skipped,totalEntries:entries};
}
export async function expandImports(files,{onProgress=()=>{}}={}){
  const flat=[],reports=[];
  for(const f of files){
    if(/\.zip$/i.test(f.name)){
      const report=await expandZip(f,{onProgress});reports.push({archive:f.name,count:report.files.length,skipped:report.skipped});flat.push(...report.files);
    }else if(mediaKind(f))flat.push(f);
  }
  return {files:flat,reports};
}
