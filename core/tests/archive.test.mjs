import test from 'node:test';
import assert from 'node:assert/strict';
import {deflateRawSync} from 'node:zlib';
import {expandZip} from '../../web/archives.js';
import {mediaKind,visibleItems} from '../../web/organizer.js';

function makeZip(name,content,method=0){
 const bytes=Buffer.from(content),compressed=method===8?deflateRawSync(bytes):bytes,filename=Buffer.from(name,'utf8');
 const local=Buffer.alloc(30);local.writeUInt32LE(0x04034b50,0);local.writeUInt16LE(20,4);local.writeUInt16LE(method,8);local.writeUInt32LE(compressed.length,18);local.writeUInt32LE(bytes.length,22);local.writeUInt16LE(filename.length,26);
 const central=Buffer.alloc(46);central.writeUInt32LE(0x02014b50,0);central.writeUInt16LE(20,4);central.writeUInt16LE(20,6);central.writeUInt16LE(method,10);central.writeUInt32LE(compressed.length,20);central.writeUInt32LE(bytes.length,24);central.writeUInt16LE(filename.length,28);
 const dir=Buffer.concat([central,filename]);const body=Buffer.concat([local,filename,compressed]);const end=Buffer.alloc(22);end.writeUInt32LE(0x06054b50,0);end.writeUInt16LE(1,8);end.writeUInt16LE(1,10);end.writeUInt32LE(dir.length,12);end.writeUInt32LE(body.length,16);
 return new File([Buffer.concat([body,dir,end])],'export.zip',{type:'application/zip'});
}
test('ZIP import preserves social source paths for uncompressed media',async()=>{
 const result=await expandZip(makeZip('Instagram/Reels/clip.mp4','video content'));
 assert.equal(result.files.length,1);
 assert.equal(result.files[0].name,'clip.mp4');
 assert.equal(result.files[0].virtualPath,'Instagram/Reels/clip.mp4');
 assert.equal(mediaKind(result.files[0]),'video');
});
test('ZIP import decodes deflated short clips',async()=>{
 const result=await expandZip(makeZip('TikTok/clip.webm','video content',8));
 assert.equal(result.files.length,1);
 assert.equal((await result.files[0].text()),'video content');
});
test('ZIP archives reject directory traversal',async()=>{
 await assert.rejects(expandZip(makeZip('../evil.mp4','no')),/Unsafe path/);
 await assert.rejects(expandZip(makeZip('/evil.mp4','no')),/Unsafe path/);
});
test('recognizes media types from phones and social services',()=>{
 for(const suffix of ['mp4','webm','mov','mkv','avi','wmv','mts','m2ts','3gp','f4v','ts'])assert.equal(mediaKind({name:`clip.${suffix}`,type:''}),'video',suffix);
 for(const suffix of ['cr3','dng','arw','nef','heic','avif','jxl','rw2'])assert.equal(mediaKind({name:`pic.${suffix}`,type:''}),'image',suffix);
});
test('100k media duplicate lookup scales linearly rather than quadratically',()=>{
 const start=performance.now();const items=Array.from({length:100_000},(_,i)=>({id:String(i),name:`IMG_${i}.jpg`,folder:'',kind:'image',tags:[],takenAt:1700000000000+i,deletedAt:null,hash:i<8000?'duplicate-'+Math.floor(i/2):'unique-'+i}));
 const matches=visibleItems(items,{view:'duplicates'});
 assert.equal(matches.length,8000);
 const duration=performance.now()-start;
 assert.ok(duration<5000,`100k comparison took ${duration}ms`);
});
