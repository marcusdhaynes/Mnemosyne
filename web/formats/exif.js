/** Minimal, bounds-checked EXIF/TIFF + XMP reader for JPEG, HEIC/AVIF, PNG and WebP.
 * Reads only the tags Mnemosyne needs for dates, devices, Live Photo pairing
 * and bursts. Malformed data returns partial results and never throws.
 */
import {ascii,u16,u32,indexOf,text} from './bytes.js';

const TYPE_SIZE={1:1,2:1,3:2,4:4,5:8,7:1,9:4,10:8};
const TAGS={make:0x010f,model:0x0110,software:0x0131,dateTime:0x0132,exifIfd:0x8769,dateTimeOriginal:0x9003,dateTimeDigitized:0x9004,offsetTime:0x9010,offsetTimeOriginal:0x9011,subSecOriginal:0x9291,userComment:0x9286,makerNote:0x927c};
// Apple MakerNote tags, per ExifTool Apple.pm (0x000b BurstUUID, 0x0011 ContentIdentifier).
const APPLE={burstUUID:0x000b,contentIdentifier:0x0011};

function readIfd(bytes,tiff,offset,little,base=tiff){
  const pos=base+offset,count=u16(bytes,pos,little),out=new Map();
  if(!offset||pos+2>bytes.length||count>500)return out;
  for(let i=0;i<count;i++){
    const at=pos+2+i*12;if(at+12>bytes.length)break;
    const tag=u16(bytes,at,little),type=u16(bytes,at+2,little),n=u32(bytes,at+4,little);
    const size=(TYPE_SIZE[type]||1)*n;
    const valueAt=size<=4?at+8:base+u32(bytes,at+8,little);
    out.set(tag,{type,count:n,size,valueAt,raw:u32(bytes,at+8,little)});
  }
  return out;
}
function readString(bytes,entry){
  if(!entry||entry.valueAt+entry.size>bytes.length)return null;
  return ascii(bytes,entry.valueAt,entry.valueAt+entry.size).replace(/\0[\s\S]*$/,'').trim()||null;
}
function readUserComment(bytes,entry){
  if(!entry||entry.size<8||entry.valueAt+entry.size>bytes.length)return null;
  const prefix=ascii(bytes,entry.valueAt,entry.valueAt+8),body=bytes.subarray(entry.valueAt+8,entry.valueAt+entry.size);
  const value=prefix.startsWith('UNICODE')?new TextDecoder('utf-16be').decode(body):ascii(body,0,body.length);
  return value.replace(/\0[\s\S]*$/,'').trim()||null;
}
function appleMakerNote(bytes,entry){
  if(!entry||entry.valueAt+16>bytes.length||ascii(bytes,entry.valueAt,entry.valueAt+10)!=='Apple iOS\0')return null;
  const base=entry.valueAt,order=ascii(bytes,base+12,base+14),little=order==='II';
  if(!little&&order!=='MM')return null;
  // ExifTool: Start => valuePtr + 14, Base => start - 14 (offsets relative to the MakerNote).
  const ifd=readIfd(bytes,base,14,little,base);
  return {contentIdentifier:readString(bytes,ifd.get(APPLE.contentIdentifier)),burstUUID:readString(bytes,ifd.get(APPLE.burstUUID))};
}
/** Parse a TIFF structure starting at `tiff` (the II/MM byte). */
export function parseTiff(bytes,tiff=0){
  const order=ascii(bytes,tiff,tiff+2),little=order==='II';
  if((!little&&order!=='MM')||u16(bytes,tiff+2,little)!==42)return null;
  const ifd0=readIfd(bytes,tiff,u32(bytes,tiff+4,little),little);
  const exifPtr=ifd0.get(TAGS.exifIfd);
  const exif=exifPtr?readIfd(bytes,tiff,exifPtr.raw,little):new Map();
  const out={
    make:readString(bytes,ifd0.get(TAGS.make)),model:readString(bytes,ifd0.get(TAGS.model)),software:readString(bytes,ifd0.get(TAGS.software)),
    dateTimeOriginal:readString(bytes,exif.get(TAGS.dateTimeOriginal)),dateTimeDigitized:readString(bytes,exif.get(TAGS.dateTimeDigitized)),dateTime:readString(bytes,ifd0.get(TAGS.dateTime)),
    offsetTimeOriginal:readString(bytes,exif.get(TAGS.offsetTimeOriginal))||readString(bytes,exif.get(TAGS.offsetTime)),
    subSecOriginal:readString(bytes,exif.get(TAGS.subSecOriginal)),userComment:readUserComment(bytes,exif.get(TAGS.userComment)),apple:null,
  };
  try{out.apple=appleMakerNote(bytes,exif.get(TAGS.makerNote));}catch{out.apple=null;}
  return out;
}

/** Extract XMP packet text from any bytes (JPEG APP1, HEIC item, PNG iTXt, MP4 uuid). */
export function findXmp(bytes){
  const start=indexOf(bytes,'<x:xmpmeta');if(start<0)return null;
  const end=indexOf(bytes,'</x:xmpmeta>',start);if(end<0)return null;
  return text(bytes.subarray(start,end+12));
}
/** Pull the few XMP properties we use. Attribute and element syntax both supported. */
export function parseXmp(xml){
  if(!xml)return null;
  const prop=name=>{const re=new RegExp(`[\\w-]+:${name}\\s*=\\s*"([^"]*)"|<[\\w-]+:${name}>([^<]*)<`);const m=xml.match(re);return m?(m[1]??m[2]).trim():null;};
  const items=[];
  for(const m of xml.matchAll(/<(?:[\w-]+:)?Item\b([^>]*?)\/?>|<rdf:li[^>]*>\s*<[\w-]+:Item\b([^>]*?)\/?>/g)){
    const attrs=m[1]??m[2]??'';const get=k=>attrs.match(new RegExp(`[\\w-]+:${k}\\s*=\\s*"([^"]*)"`))?.[1]??null;
    const mime=get('Mime');if(mime)items.push({mime,semantic:get('Semantic'),length:Number(get('Length')||0),padding:Number(get('Padding')||0)});
  }
  return {
    motionPhoto:prop('MotionPhoto'),motionPhotoVersion:prop('MotionPhotoVersion'),presentationTimestampUs:prop('MotionPhotoPresentationTimestampUs'),
    microVideo:prop('MicroVideo'),microVideoOffset:prop('MicroVideoOffset'),microVideoVersion:prop('MicroVideoVersion'),
    containerItems:items,createDate:prop('DateCreated')||prop('CreateDate')||prop('DateTimeOriginal'),
  };
}

/** JPEG: walk APPn segments; returns {tiff:{bytes,offset}|null, xmp}. */
export function jpegMetadata(bytes){
  let exif=null,xmp=null;
  for(let p=2;p+4<=bytes.length;){
    if(bytes[p]!==0xff)break;
    const marker=bytes[p+1];
    if(marker===0xd8||marker===0x01||(marker>=0xd0&&marker<=0xd7)){p+=2;continue;}
    if(marker===0xda||marker===0xd9)break;
    const size=u16(bytes,p+2);if(size<2)break;
    const start=p+4,end=Math.min(bytes.length,p+2+size);
    if(marker===0xe1&&ascii(bytes,start,start+6)==='Exif\0\0'&&!exif)exif=parseTiff(bytes.subarray(start+6,end),0);
    else if(marker===0xe1&&ascii(bytes,start,start+29)==='http://ns.adobe.com/xap/1.0/\0'&&!xmp)xmp=text(bytes.subarray(start+29,end));
    p+=2+size;
  }
  return {exif,xmp:xmp||findXmp(bytes)};
}
/** PNG: eXIf chunk (raw TIFF) and iTXt XML:com.adobe.xmp. */
export function pngMetadata(bytes){
  let exif=null,xmp=null;
  for(let p=8;p+12<=bytes.length;){
    const len=u32(bytes,p),type=ascii(bytes,p+4,p+8),data=bytes.subarray(p+8,Math.min(bytes.length,p+8+len));
    if(type==='eXIf')exif=parseTiff(data,ascii(data,0,6)==='Exif\0\0'?6:0);
    else if(type==='iTXt'&&ascii(data,0,18)==='XML:com.adobe.xmp\0')xmp=findXmp(data);
    if(type==='IEND')break;p+=12+len;
  }
  return {exif,xmp};
}
/** WebP: RIFF chunks EXIF and "XMP ". */
export function webpMetadata(bytes){
  let exif=null,xmp=null;
  for(let p=12;p+8<=bytes.length;){
    const type=ascii(bytes,p,p+4),len=u32(bytes,p+4,true),data=bytes.subarray(p+8,Math.min(bytes.length,p+8+len));
    if(type==='EXIF')exif=parseTiff(data,ascii(data,0,6)==='Exif\0\0'?6:0);
    if(type==='XMP ')xmp=text(data);
    p+=8+len+(len&1);
  }
  return {exif,xmp};
}

/** HEIC/AVIF: locate the Exif and XMP items through meta/iinf/iloc (construction method 0). */
export async function heifMetadata(source,head){
  const items=heifItems(head);let exif=null,xmp=null;
  for(const item of items){
    if(!item.extent||item.extent.length>8*1024*1024)continue;
    if(item.type==='Exif'&&!exif){
      const data=await source.read(item.extent.offset,item.extent.offset+item.extent.length);
      const skip=4+u32(data,0);
      exif=parseTiff(data,ascii(data,skip,skip+6)==='Exif\0\0'?skip+6:skip);
    }else if(item.type==='mime'&&/rdf\+xml|xmp/i.test(item.contentType||'')&&!xmp){
      xmp=text(await source.read(item.extent.offset,item.extent.offset+item.extent.length));
    }
  }
  if(!exif){const i=indexOf(head,'Exif\0\0');if(i>=0)exif=parseTiff(head,i+6);}
  return {exif,xmp:xmp||findXmp(head)};
}
function boxes(bytes,start,end){
  const out=[];
  for(let p=start;p+8<=end;){
    let size=u32(bytes,p),header=8;const type=ascii(bytes,p+4,p+8);
    if(size===1){size=u32(bytes,p+8)*0x100000000+u32(bytes,p+12);header=16;}else if(size===0)size=end-p;
    if(size<header||p+size>end)break;
    out.push({type,start:p,body:p+header,end:p+size});p+=size;
  }
  return out;
}
export function heifItems(head){
  const meta=boxes(head,0,head.length).find(b=>b.type==='meta');if(!meta)return [];
  const children=boxes(head,meta.body+4,meta.end);
  const iinf=children.find(b=>b.type==='iinf'),iloc=children.find(b=>b.type==='iloc');
  if(!iinf||!iloc)return [];
  const infos=new Map();
  const iv=head[iinf.body];
  for(const infe of boxes(head,iinf.body+4+(iv===0?2:4),iinf.end)){
    if(infe.type!=='infe')continue;
    const v=head[infe.body];if(v<2)continue;
    let p=infe.body+4;const id=v===2?u16(head,p):u32(head,p);p+=v===2?2:4;p+=2;
    const type=ascii(head,p,p+4);p+=4;
    const nameEnd=head.indexOf(0,p);p=nameEnd<0?infe.end:nameEnd+1;
    let contentType=null;if(type==='mime'){const e=head.indexOf(0,p);contentType=ascii(head,p,e<0?infe.end:e);}
    infos.set(id,{id,type,contentType});
  }
  const v=head[iloc.body];let p=iloc.body+4;
  const offsetSize=head[p]>>4,lengthSize=head[p]&15,baseSize=head[p+1]>>4,indexSize=v>0?head[p+1]&15:0;p+=2;
  const read=n=>{let x=0;for(let i=0;i<n;i++)x=x*256+head[p+i];p+=n;return x;};
  const count=v<2?read(2):read(4);const out=[];
  for(let i=0;i<count&&p<iloc.end;i++){
    const id=v<2?read(2):read(4);let method=0;if(v>0)method=read(2)&15;
    read(2);const base=read(baseSize);const extents=read(2);let first=null;
    for(let e=0;e<extents;e++){if(indexSize)read(indexSize);const off=read(offsetSize),len=read(lengthSize);if(!first)first={offset:base+off,length:len};}
    const info=infos.get(id);if(info&&method===0)out.push({...info,extent:first});
  }
  return out;
}
