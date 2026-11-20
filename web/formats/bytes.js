/** Byte-level helpers shared by the media-format readers.
 * Everything here is dependency-free and works in browsers, Node 22+ and a
 * Tauri webview. Readers only ever *read* slices; originals are never modified.
 */
const latin1=new TextDecoder('latin1');
const utf8=new TextDecoder('utf-8');

/** Normalise any supported input into a small random-access source.
 * Accepts a browser/Node `File`/`Blob`, or a descriptor
 * `{path,name?,size,lastModified?,read(start,end)->Promise<Uint8Array>}`, or
 * `{path,bytes:Uint8Array}` (tests and native bridges). */
export function toSource(input){
  if(!input)throw new TypeError('toSource: missing file');
  const path=normalizePath(input.path||input.virtualPath||input.webkitRelativePath||input.name||'');
  const name=input.name||path.split('/').at(-1)||'';
  const mtime=Number.isFinite(input.lastModified)?input.lastModified:(Number.isFinite(input.mtimeMs)?input.mtimeMs:null);
  if(input.bytes instanceof Uint8Array){
    const bytes=input.bytes;
    return {input,path,name,size:bytes.length,mtime,read:async(start,end)=>bytes.subarray(clamp(start,bytes.length),clamp(end,bytes.length))};
  }
  if(typeof input.read==='function'){
    return {input,path,name,size:input.size,mtime,read:async(start,end)=>input.read(clamp(start,input.size),clamp(end,input.size))};
  }
  if(typeof input.slice==='function'&&Number.isFinite(input.size)){
    return {input,path,name,size:input.size,mtime,read:async(start,end)=>new Uint8Array(await input.slice(clamp(start,input.size),clamp(end,input.size)).arrayBuffer())};
  }
  throw new TypeError(`toSource: unsupported input for ${path||'file'}`);
}
const clamp=(n,max)=>Math.max(0,Math.min(Number.isFinite(n)?n:max,max));
export function normalizePath(path){return String(path).replaceAll('\\','/').replace(/^\.\//,'');}
export function dirname(path){const i=path.lastIndexOf('/');return i<0?'':path.slice(0,i);}
export function basename(path){return path.slice(path.lastIndexOf('/')+1);}
export function stem(name){const i=name.lastIndexOf('.');return i<=0?name:name.slice(0,i);}
export function ext(name){const i=name.lastIndexOf('.');return i<0?'':name.slice(i+1).toLowerCase();}

export const ascii=(bytes,start,end)=>latin1.decode(bytes.subarray(start,end));
export const text=bytes=>utf8.decode(bytes);
export function u16(bytes,p,little=false){return p+2>bytes.length?0:little?bytes[p]|bytes[p+1]<<8:bytes[p]<<8|bytes[p+1];}
export function u32(bytes,p,little=false){if(p+4>bytes.length)return 0;return little?(bytes[p]|bytes[p+1]<<8|bytes[p+2]<<16|bytes[p+3]<<24)>>>0:(bytes[p]*0x1000000+(bytes[p+1]<<16|bytes[p+2]<<8|bytes[p+3]));}
export function u64(bytes,p){return u32(bytes,p)*0x100000000+u32(bytes,p+4);}
/** First index of an ASCII needle in bytes, or -1. */
export function indexOf(bytes,needle,from=0){
  const n=typeof needle==='string'?[...needle].map(c=>c.charCodeAt(0)):needle;
  outer:for(let i=Math.max(0,from);i<=bytes.length-n.length;i++){if(bytes[i]!==n[0])continue;for(let j=1;j<n.length;j++)if(bytes[i+j]!==n[j])continue outer;return i;}
  return -1;
}
/** Search a whole source in chunks (bounded memory). Returns absolute offset or -1. */
export async function findInSource(source,needle,{from=0,to=source.size,chunk=1<<20}={}){
  for(let start=from;start<to;start+=chunk){
    const end=Math.min(to,start+chunk+needle.length-1);
    const hit=indexOf(await source.read(start,end),needle);
    if(hit>=0)return start+hit;
  }
  return -1;
}
/** Magic-byte sniffing; extension is only a fallback hint. */
export function sniff(head,name=''){
  if(head[0]===0xff&&head[1]===0xd8&&head[2]===0xff)return 'jpeg';
  if(head[0]===0x89&&ascii(head,1,4)==='PNG')return 'png';
  if(ascii(head,0,4)==='RIFF'&&ascii(head,8,12)==='WEBP')return 'webp';
  if(ascii(head,0,4)==='GIF8')return 'gif';
  if(ascii(head,4,8)==='ftyp'){
    const brands=[ascii(head,8,12),...Array.from({length:Math.max(0,Math.min(u32(head,0),64)-16)/4|0},(_,i)=>ascii(head,16+i*4,20+i*4))];
    if(brands.some(b=>['heic','heix','hevc','hevx','heim','heis','mif1','msf1'].includes(b))&&!brands.includes('qt  ')&&!/^(isom|mp4|M4V|avc1|iso)/.test(brands[0]))return 'heic';
    if(brands.some(b=>b==='avif'||b==='avis'))return 'avif';
    if(brands[0]==='crx ')return 'cr3';
    if(brands[0]==='qt  ')return 'quicktime';
    return 'mp4';
  }
  if(['moov','mdat','wide','free','skip'].includes(ascii(head,4,8)))return 'quicktime';
  if((head[0]===0x49&&head[1]===0x49&&head[2]===42)||(head[0]===0x4d&&head[1]===0x4d&&head[3]===42))return 'tiff';
  if(head[0]===0x1a&&head[1]===0x45&&head[2]===0xdf&&head[3]===0xa3)return 'matroska';
  const e=ext(name);
  return ({jpg:'jpeg',jpeg:'jpeg',heic:'heic',heif:'heic',png:'png',mov:'quicktime',mp4:'mp4',m4v:'mp4',webp:'webp'})[e]||'unknown';
}
export const isVideoContainer=c=>c==='mp4'||c==='quicktime'||c==='matroska';
