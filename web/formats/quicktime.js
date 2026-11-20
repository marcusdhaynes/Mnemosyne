/** ISO-BMFF / QuickTime reader for MOV and MP4: only what the format layer needs
 * (Live Photo ContentIdentifier, creation dates, duration, size, frame rate,
 * Android capture fps). Reads box headers and the `moov` box; never the media data.
 */
import {ascii,u16,u32,u64,text} from './bytes.js';
const MAX_MOOV=64*1024*1024;
const MAC_EPOCH_OFFSET=2082844800;
export function childBoxes(bytes,start,end){
  const out=[];
  for(let p=start;p+8<=end;){
    let size=u32(bytes,p),header=8;const type=ascii(bytes,p+4,p+8);
    if(size===1){size=u64(bytes,p+8);header=16;}else if(size===0)size=end-p;
    if(size<header||p+size>end)break;
    out.push({type,start:p,body:p+header,end:p+size});p+=size;
  }
  return out;
}
const find=(bytes,box,type)=>childBoxes(bytes,box.body,box.end).find(b=>b.type===type);
const findAll=(bytes,box,type)=>childBoxes(bytes,box.body,box.end).filter(b=>b.type===type);
/** Walk top-level boxes using small header reads; returns [{type,start,size}]. */
export async function topLevelBoxes(source,limit=4096){
  const out=[];
  for(let p=0;p+8<=source.size&&out.length<limit;){
    const h=await source.read(p,p+16);let size=u32(h,0),header=8;
    if(size===1){size=u64(h,8);header=16;}else if(size===0)size=source.size-p;
    if(size<header)break;
    out.push({type:ascii(h,4,8),start:p,size,header});p+=size;
  }
  return out;
}
function readKeys(bytes,meta){
  const keys=find(bytes,meta,'keys'),ilst=find(bytes,meta,'ilst');const out={};
  if(!keys||!ilst)return out;
  const names=[];
  for(let p=keys.body+8,i=0,n=u32(bytes,keys.body+4);i<n&&p+8<=keys.end;i++){const size=u32(bytes,p);if(size<8)break;names.push(ascii(bytes,p+8,p+size));p+=size;}
  for(const item of childBoxes(bytes,ilst.body,ilst.end)){
    const index=u32(bytes,item.start+4),name=names[index-1];if(!name)continue;
    const data=find(bytes,item,'data');if(!data)continue;
    const type=u32(bytes,data.body),value=bytes.subarray(data.body+8,data.end);
    out[name]=type===1?text(value):type===23&&value.length===4?new DataView(value.buffer,value.byteOffset,4).getFloat32(0):type===24&&value.length===8?new DataView(value.buffer,value.byteOffset,8).getFloat64(0):type===21||type===22?value.reduce((a,b)=>a*256+b,0):text(value);
  }
  return out;
}
function trackInfo(bytes,trak){
  const mdia=find(bytes,trak,'mdia');if(!mdia)return null;
  const hdlr=find(bytes,mdia,'hdlr');const handler=hdlr?ascii(bytes,hdlr.body+8,hdlr.body+12):null;
  const mdhd=find(bytes,mdia,'mdhd');let timescale=0,duration=0;
  if(mdhd){const v=bytes[mdhd.body];timescale=v===1?u32(bytes,mdhd.body+20):u32(bytes,mdhd.body+12);duration=v===1?u64(bytes,mdhd.body+24):u32(bytes,mdhd.body+16);}
  let samples=0,sampleTime=0;
  const stts=find(bytes,find(bytes,find(bytes,mdia,'minf')||mdia,'stbl')||mdia,'stts');
  if(stts){for(let i=0,n=Math.min(u32(bytes,stts.body+4),100000);i<n;i++){const at=stts.body+8+i*8;if(at+8>stts.end)break;const c=u32(bytes,at),d=u32(bytes,at+4);samples+=c;sampleTime+=c*d;}}
  const tkhd=find(bytes,trak,'tkhd');let width=0,height=0,rotation=0;
  if(tkhd){
    const v=bytes[tkhd.body],m=tkhd.body+(v===1?52:40);
    const a=u32(bytes,m)|0,b=u32(bytes,m+4)|0;
    rotation=a===0&&b>0?90:a===0&&b<0?270:a<0?180:0;
    width=u32(bytes,tkhd.end-8)/65536;height=u32(bytes,tkhd.end-4)/65536;
  }
  const fps=handler==='vide'&&sampleTime>0&&timescale>0?Math.round(samples*timescale/sampleTime*100)/100:null;
  return {handler,timescale,duration,width,height,rotation,fps};
}
/** Parse MOV/MP4 metadata. */
export async function videoMetadata(source){
  const top=await topLevelBoxes(source);
  const moovBox=top.find(b=>b.type==='moov');
  if(!moovBox||moovBox.size>MAX_MOOV)return {brands:null,durationSec:null};
  const bytes=await source.read(moovBox.start,moovBox.start+moovBox.size);
  const moov={body:moovBox.header,end:bytes.length};
  const out={durationSec:null,createdUtc:null,width:null,height:null,rotation:0,fps:null,keys:{},hasAudio:false};
  const mvhd=find(bytes,moov,'mvhd');
  if(mvhd){
    const v=bytes[mvhd.body];const created=v===1?u64(bytes,mvhd.body+4):u32(bytes,mvhd.body+4);
    const timescale=v===1?u32(bytes,mvhd.body+20):u32(bytes,mvhd.body+12),duration=v===1?u64(bytes,mvhd.body+24):u32(bytes,mvhd.body+16);
    if(created>MAC_EPOCH_OFFSET)out.createdUtc=(created-MAC_EPOCH_OFFSET)*1000;
    if(timescale)out.durationSec=duration/timescale;
  }
  for(const trak of findAll(bytes,moov,'trak')){
    const t=trackInfo(bytes,trak);if(!t)continue;
    if(t.handler==='soun')out.hasAudio=true;
    if(t.handler==='vide'&&out.width===null){Object.assign(out,{width:t.width,height:t.height,rotation:t.rotation,fps:t.fps});}
  }
  // Apple puts mdta keys in moov/meta (QuickTime meta: no version/flags); some writers use udta/meta.
  for(const meta of [find(bytes,moov,'meta'),find(bytes,find(bytes,moov,'udta')||{body:0,end:0},'meta')].filter(Boolean)){
    const isFull=u32(bytes,meta.body)===0&&ascii(bytes,meta.body+8,meta.body+12)==='hdlr';
    Object.assign(out.keys,readKeys(bytes,isFull?{body:meta.body+4,end:meta.end}:meta));
  }
  return out;
}
/** Find where an MP4 that starts at `start` ends, by walking its top-level boxes. */
export async function mp4Extent(source,start,limit=source.size){
  let p=start;const head=await source.read(start,start+12);
  if(ascii(head,4,8)!=='ftyp')return null;
  while(p+8<=limit){
    const h=await source.read(p,p+16);let size=u32(h,0);const type=ascii(h,4,8);
    if(size===1)size=u64(h,8);else if(size===0)size=limit-p;
    if(size<8||!/^[\x20-\x7e]{4}$/.test(type)||p+size>limit)break;
    p+=size;
  }
  return p>start?{offset:start,length:p-start}:null;
}
