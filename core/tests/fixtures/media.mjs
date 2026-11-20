/** Synthetic media fixtures (no real photos). Each builder emits the smallest
 * structurally valid bytes the format layer needs to see. */
const enc=s=>Buffer.from(s,'latin1');
const be16=n=>{const b=Buffer.alloc(2);b.writeUInt16BE(n);return b;};
const be32=n=>{const b=Buffer.alloc(4);b.writeUInt32BE(n>>>0);return b;};
const cat=(...parts)=>Buffer.concat(parts.map(p=>typeof p==='string'?enc(p):p));
/** Big-endian IFD. entries: [{tag,type:2|4|7,value:string|number|Buffer}]; offsets relative to `base`. */
function ifd(entries,start,base=0){
  const sorted=[...entries].sort((a,b)=>a.tag-b.tag);
  const head=Buffer.alloc(2+sorted.length*12+4);head.writeUInt16BE(sorted.length,0);
  let data=Buffer.alloc(0);const dataStart=start+head.length;
  sorted.forEach((e,i)=>{
    const at=2+i*12;let bytes=e.type===2?cat(e.value,'\0'):e.type===4?be32(e.value):e.value;
    head.writeUInt16BE(e.tag,at);head.writeUInt16BE(e.type,at+2);head.writeUInt32BE(e.type===4?1:bytes.length,at+4);
    if(bytes.length<=4){bytes.copy(head,at+8);}else{head.writeUInt32BE(dataStart+data.length-base,at+8);data=cat(data,bytes,bytes.length&1?Buffer.alloc(1):Buffer.alloc(0));}
  });
  return cat(head,data);
}
export function appleMakerNote({contentIdentifier,burstUUID}={}){
  const entries=[];
  if(burstUUID)entries.push({tag:0x000b,type:2,value:burstUUID});
  if(contentIdentifier)entries.push({tag:0x0011,type:2,value:contentIdentifier});
  return cat('Apple iOS\0',be16(1),'MM',ifd(entries,14,0));
}
/** TIFF (MM) with IFD0 Make/Model + Exif IFD dates/offset/subsec/UserComment/MakerNote. */
export function tiff({make,model,date,offset,subsec,userComment,apple}={}){
  const exifEntries=[];
  if(date)exifEntries.push({tag:0x9003,type:2,value:date});
  if(offset)exifEntries.push({tag:0x9011,type:2,value:offset});
  if(subsec)exifEntries.push({tag:0x9291,type:2,value:subsec});
  if(userComment)exifEntries.push({tag:0x9286,type:7,value:cat('ASCII\0\0\0',userComment)});
  if(apple)exifEntries.push({tag:0x927c,type:7,value:appleMakerNote(apple)});
  const ifd0Entries=[{tag:0x8769,type:4,value:0}];
  if(make)ifd0Entries.push({tag:0x010f,type:2,value:make});
  if(model)ifd0Entries.push({tag:0x0110,type:2,value:model});
  const first=ifd(ifd0Entries,8);const exifStart=8+first.length;
  const real=ifd(ifd0Entries.map(e=>e.tag===0x8769?{...e,value:exifStart}:e),8);
  return cat('MM',be16(42),be32(8),real,ifd(exifEntries,exifStart));
}
const seg=(marker,body)=>cat(Buffer.from([0xff,marker]),be16(body.length+2),body);
export function jpeg({exif,xmp,tail}={}){
  return cat(Buffer.from([0xff,0xd8]),exif?seg(0xe1,cat('Exif\0\0',tiff(exif))):'',xmp?seg(0xe1,cat('http://ns.adobe.com/xap/1.0/\0',xmp)):'',seg(0xdb,Buffer.alloc(4)),Buffer.from([0xff,0xd9]),tail||'');
}
export const box=(type,...body)=>{const b=cat(...body);return cat(be32(b.length+8),type,b);};
const full=(type,v,...body)=>box(type,Buffer.from([v,0,0,0]),...body);
/** HEIC with an Exif item located via iloc (version 0, 4-byte offsets/lengths). */
export function heic({exif}){
  const exifPayload=cat(be32(6),'Exif\0\0',tiff(exif));
  const ftyp=box('ftyp','heic',be32(0),'mif1','heic');
  const infe=full('infe',2,be16(1),be16(0),'Exif','\0');
  const iinf=full('iinf',0,be16(1),infe);
  const build=offset=>full('iloc',0,Buffer.from([0x44,0x00]),be16(1),be16(1),be16(0),be16(1),be32(offset),be32(exifPayload.length));
  const meta=o=>full('meta',0,full('hdlr',0,be32(0),'pict',Buffer.alloc(13)),iinf,build(o));
  const pre=cat(ftyp,meta(0));const offset=pre.length+8;
  return cat(ftyp,meta(offset),box('mdat',exifPayload));
}
export function png({exif}){
  const chunk=(type,data)=>cat(be32(data.length),type,data,be32(0));
  return cat(Buffer.from([0x89]),'PNG\r\n\x1a\n',chunk('IHDR',Buffer.alloc(13)),chunk('eXIf',tiff(exif)),chunk('IEND',Buffer.alloc(0)));
}
/** QuickTime/MP4. keys: {name:value} mdta strings (or {float32}). */
export function movie({brand='qt  ',created=null,durationSec=3,timescale=600,fps=30,width=1920,height=1080,keys={},audio=false}={}){
  const mac=created?Math.floor(created/1000)+2082844800:0;
  const mvhd=full('mvhd',0,be32(mac),be32(mac),be32(timescale),be32(Math.round(durationSec*timescale)),Buffer.alloc(80));
  const frames=Math.max(1,Math.round(durationSec*fps));
  const delta=Math.round(timescale/fps);
  const tkhd=full('tkhd',0,Buffer.alloc(36),be32(0x10000),be32(0),be32(0),be32(0),be32(0x10000),be32(0),be32(0),be32(0),be32(0x40000000),be32(width*65536),be32(height*65536));
  const track=(handler)=>box('trak',tkhd,box('mdia',full('mdhd',0,be32(0),be32(0),be32(timescale),be32(frames*delta),be32(0)),full('hdlr',0,be32(0),handler,Buffer.alloc(13)),box('minf',box('stbl',full('stts',0,be32(1),be32(frames),be32(delta))))));
  const names=Object.keys(keys);
  const keysBox=full('keys',0,be32(names.length),...names.map(n=>cat(be32(n.length+8),'mdta',n)));
  const ilst=box('ilst',...names.map((n,i)=>{const v=keys[n];const data=typeof v==='number'?(()=>{const b=Buffer.alloc(4);b.writeFloatBE(v);return box('data',be32(23),be32(0),b);})():box('data',be32(1),be32(0),v);return cat(be32(data.length+8),be32(i+1),data);}));
  const meta=names.length?box('meta',full('hdlr',0,be32(0),'mdta',Buffer.alloc(13)),keysBox,ilst):Buffer.alloc(0);
  return cat(box('ftyp',brand,be32(0),brand),box('mdat',Buffer.alloc(16)),box('moov',mvhd,track('vide'),audio?track('soun'):'',meta));
}
export const tinyMp4=()=>movie({brand:'isom',durationSec:1.5});
/** Google Motion Photo 1.0: JPEG + XMP GContainer directory + appended MP4. */
export function googleMotionPhoto({legacy=false}={}){
  const video=tinyMp4();
  const xmp=legacy
    ?`<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF><rdf:Description GCamera:MicroVideo="1" GCamera:MicroVideoVersion="1" GCamera:MicroVideoOffset="${video.length}"/></rdf:RDF></x:xmpmeta>`
    :`<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF><rdf:Description Camera:MotionPhoto="1" Camera:MotionPhotoVersion="1" Camera:MotionPhotoPresentationTimestampUs="500000"><Container:Directory><rdf:Seq><rdf:li rdf:parseType="Resource"><Container:Item Item:Mime="image/jpeg" Item:Semantic="Primary" Item:Length="0" Item:Padding="0"/></rdf:li><rdf:li rdf:parseType="Resource"><Container:Item Item:Mime="video/mp4" Item:Semantic="MotionPhoto" Item:Length="${video.length}"/></rdf:li></rdf:Seq></Container:Directory></rdf:Description></rdf:RDF></x:xmpmeta>`;
  return {bytes:jpeg({exif:{make:'Google',model:'Pixel 8',date:'2024:05:01 10:00:00'},xmp,tail:video}),video};
}
/** Samsung: JPEG + "MotionPhoto_Data" + MP4 + SEF trailer (SEFH … SEFT). */
export function samsungMotionPhoto(){
  const video=tinyMp4();
  const trailer=cat('SEFH',be32(107),be32(1),Buffer.alloc(12),be32(24),'SEFT');
  return {bytes:jpeg({exif:{make:'samsung',model:'SM-S918B',date:'2024:06:01 09:00:00'},tail:cat('MotionPhoto_Data',video,trailer)}),video};
}
export const file=(path,bytes,lastModified=Date.UTC(2025,0,1))=>{const f=new File([bytes],path.split('/').at(-1),{lastModified});Object.defineProperty(f,'virtualPath',{value:path,enumerable:true});return f;};
export const json=(path,value)=>file(path,Buffer.from(JSON.stringify(value)));
