import test from 'node:test';
import assert from 'node:assert/strict';
import {deflateRawSync} from 'node:zlib';
import {expandZip,expandImports} from '../../web/archives.js';
import {detectExportKind,enumerateImportFiles,isSidecarName,connectCloud,COMMIT_DEFAULT} from '../../web/source-adapters.js';
import {SOURCES,COMMIT_DEFAULT as SOURCE_COMMIT_DEFAULT} from '../../web/sources.js';

function makeZip(entries, method=0){
  const parts=[], centrals=[];
  let offset=0;
  for(const [name,content] of entries){
    const bytes=Buffer.from(content), compressed=method===8?deflateRawSync(bytes):bytes, filename=Buffer.from(name,'utf8');
    const local=Buffer.alloc(30);local.writeUInt32LE(0x04034b50,0);local.writeUInt16LE(20,4);local.writeUInt16LE(method,8);local.writeUInt32LE(compressed.length,18);local.writeUInt32LE(bytes.length,22);local.writeUInt16LE(filename.length,26);
    const central=Buffer.alloc(46);central.writeUInt32LE(0x02014b50,0);central.writeUInt16LE(20,4);central.writeUInt16LE(20,6);central.writeUInt16LE(method,10);central.writeUInt32LE(compressed.length,20);central.writeUInt32LE(bytes.length,24);central.writeUInt16LE(filename.length,28);
    const body=Buffer.concat([local,filename,compressed]);
    parts.push(body);
    centrals.push(Buffer.concat([central,filename]));
    offset+=body.length;
  }
  const dir=Buffer.concat(centrals);
  const body=Buffer.concat(parts);
  const end=Buffer.alloc(22);end.writeUInt32LE(0x06054b50,0);end.writeUInt16LE(entries.length,8);end.writeUInt16LE(entries.length,10);end.writeUInt32LE(dir.length,12);end.writeUInt32LE(body.length,16);
  return new File([Buffer.concat([body,dir,end])],'export.zip',{type:'application/zip'});
}

test('source registry still exposes every previous source id',()=>{
  const ids=SOURCES.map(s=>s.id);
  for(const id of ['device','folder','zip','google-drive','google-photos','apple-photos','onedrive','dropbox','android','nas']){
    assert.ok(ids.includes(id),id);
  }
  assert.equal(SOURCE_COMMIT_DEFAULT,'album_only');
  assert.equal(COMMIT_DEFAULT,'album_only');
  assert.equal(SOURCES.find(s=>s.id==='google-drive').availability,'setup');
});

test('ZIP import keeps media in files and lists sidecar JSON separately',async()=>{
  const zip=makeZip([
    ['Takeout/Google Photos/IMG_0001.jpg','jpeg-bytes'],
    ['Takeout/Google Photos/IMG_0001.jpg.json','{"photoTakenTime":{"timestamp":"1704067200"}}'],
    ['Takeout/Mail/ignored.mbox','nope']
  ]);
  const result=await expandZip(zip);
  assert.equal(result.files.length,1);
  assert.equal(result.files[0].virtualPath,'Takeout/Google Photos/IMG_0001.jpg');
  assert.equal(result.sidecars.length,1);
  assert.equal(result.sidecars[0].virtualPath,'Takeout/Google Photos/IMG_0001.jpg.json');
  const enumerated=enumerateImportFiles(result.files,{sidecars:result.sidecars});
  const media=enumerated.find(x=>x.role==='media');
  assert.equal(media.exportKind,'takeout');
  assert.equal(media.sidecarPaths.length,1);
  assert.equal(media.format,null);
});

test('expandImports keeps export sidecars for enumeration',async()=>{
  const zip=makeZip([
    ['your_instagram_activity/media/posts/202401/clip.mp4','video content'],
    ['your_instagram_activity/media/posts_1.json','[{"media":[{"uri":"media/posts/202401/clip.mp4","creation_timestamp":1704067200}]}]']
  ]);
  const result=await expandImports([zip]);
  assert.equal(result.files.length,1);
  const enumerated=enumerateImportFiles(result.files,{sidecars:result.sidecars});
  assert.equal(detectExportKind(enumerated.map(x=>x.path)),'instagram');
  assert.ok(enumerated.some(x=>x.role==='sidecar'));
});

test('sidecar names are recognized without reading them',()=>{
  assert.equal(isSidecarName('IMG_0001.jpg.json'),true);
  assert.equal(isSidecarName('IMG_0001.jpg'),false);
});

test('cloud connectors throw documented stubs and make no requests',()=>{
  for(const id of ['google-drive','google-photos','onedrive','dropbox','apple-photos']){
    assert.throws(()=>connectCloud(id),/not connected|no public/i);
  }
});
