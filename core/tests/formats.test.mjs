import test from 'node:test';
import assert from 'node:assert/strict';
import {resolveMedia,inspectFile,extractMotionVideo,dateFromFilename,fixMojibake} from '../../web/formats/index.js';
import {takeoutMatcher,parseMetaExport,parseSnapchatJson,parseSnapchatHtml,parseExifDate} from '../../web/formats/dates.js';
import {jpeg,heic,png,movie,googleMotionPhoto,samsungMotionPhoto,file,json} from './fixtures/media.mjs';

const UTC=(...a)=>Date.UTC(...a);
const localMs=s=>new Date(s).getTime();

test('EXIF fallback reads JPEG, HEIC and PNG dates plus Apple MakerNote tags',async()=>{
  const exif={make:'Apple',model:'iPhone 15',date:'2024:03:02 11:22:33',offset:'-05:00',apple:{contentIdentifier:'CID-1',burstUUID:'BURST-1'}};
  for(const [name,bytes] of [['a.jpg',jpeg({exif})],['a.heic',heic({exif})],['a.png',png({exif})]]){
    const info=await inspectFile(file(name,bytes));
    assert.equal(info.embeddedDates[0]?.takenAt,UTC(2024,2,2,16,22,33),name);
    assert.equal(info.contentIdentifier,'CID-1',name);assert.equal(info.burstUUID,'BURST-1',name);
  }
  assert.equal(parseExifDate('0000:00:00 00:00:00'),null);
});
test('QuickTime: ContentIdentifier, creationdate, duration, fps',async()=>{
  const info=await inspectFile(file('IMG_1.MOV',movie({durationSec:2.5,fps:30,keys:{'com.apple.quicktime.content.identifier':'CID-9','com.apple.quicktime.creationdate':'2024-03-02T11:22:34-0500'}})));
  assert.equal(info.mediaKind,'video');assert.equal(info.contentIdentifier,'CID-9');
  assert.equal(info.embeddedDates.find(d=>d.source==='quicktime').takenAt,UTC(2024,2,2,16,22,34));
  assert.equal(info.video.durationSec,2.5);assert.equal(info.video.fps,30);assert.equal(info.video.width,1920);
});
test('Live Photo pairs by ContentIdentifier even when renamed, and keeps the video',async()=>{
  const still=file('DCIM/IMG_0001.HEIC',heic({exif:{date:'2024:03:02 11:22:33',apple:{contentIdentifier:'CID-1'}}}));
  const clip=file('DCIM/IMG_0001 (1).MOV',movie({durationSec:2.9,keys:{'com.apple.quicktime.content.identifier':'CID-1'}}));
  const items=await resolveMedia([clip,still]);
  assert.equal(items.length,1);
  assert.equal(items[0].kind,'live-photo');assert.equal(items[0].livePairing,'content-identifier');
  assert.equal(items[0].primaryPath,'DCIM/IMG_0001.HEIC');assert.equal(items[0].pairedVideoPath,'DCIM/IMG_0001 (1).MOV');
  assert.deepEqual(items[0].files.map(f=>f.role),['primary','live-video']);
});
test('Live Photo falls back to basename + close timestamps, and refuses distant ones',async()=>{
  const t=UTC(2024,0,1,12);
  const near=await resolveMedia([file('x/IMG_2.JPG',jpeg(),t),file('x/IMG_2.MOV',movie({durationSec:3}),t+1500)]);
  assert.equal(near.length,1);assert.equal(near[0].livePairing,'basename-time');
  const far=await resolveMedia([file('x/IMG_3.JPG',jpeg(),t),file('x/IMG_3.MOV',movie({durationSec:3}),t+3600_000)]);
  assert.equal(far.length,2);
  const long=await resolveMedia([file('x/IMG_4.JPG',jpeg(),t),file('x/IMG_4.MOV',movie({durationSec:40}),t)]);
  assert.equal(long.length,2,'a 40 s clip is not a Live Photo half');
});
test('Google Motion Photo (GContainer and legacy MicroVideo) exposes the embedded MP4',async()=>{
  for(const legacy of [false,true]){
    const {bytes,video}=googleMotionPhoto({legacy});const f=file('PXL_20240501_100000000.MP.jpg',bytes);
    const [item]=await resolveMedia([f]);
    assert.equal(item.kind,'motion-photo');
    assert.equal(item.motionVideo.source,legacy?'xmp-microvideo':'xmp-container');
    assert.equal(item.motionVideo.offset,bytes.length-video.length);
    const blob=await extractMotionVideo(f,item.motionVideo);
    assert.deepEqual(Buffer.from(await blob.arrayBuffer()),video);
  }
});
test('Samsung Motion Photo found via MotionPhoto_Data before the SEF trailer',async()=>{
  const {bytes,video}=samsungMotionPhoto();const f=file('20240601_090000.jpg',bytes);
  const [item]=await resolveMedia([f]);
  assert.equal(item.kind,'motion-photo');assert.equal(item.motionVideo.source,'samsung-sef');
  assert.equal(item.motionVideo.length,video.length);
  assert.deepEqual(Buffer.from(await (await extractMotionVideo(f,item.motionVideo)).arrayBuffer()),video);
});
test('A plain JPEG is not a Motion Photo',async()=>{const [item]=await resolveMedia([file('a.jpg',jpeg({exif:{date:'2024:01:01 00:00:00'}}))]);assert.equal(item.kind,'photo');assert.equal(item.motionVideo,null);});
test('Bursts group by Apple BurstUUID, filename patterns and tight timing; cover chosen',async()=>{
  const apple=await resolveMedia([1,2,3].map(i=>file(`b/IMG_00${i}.JPG`,jpeg({exif:{make:'Apple',model:'iPhone',date:`2024:01:01 10:00:0${i}`,apple:{burstUUID:'U-1'}}}))));
  assert.equal(apple.length,1);assert.equal(apple[0].kind,'burst');assert.equal(apple[0].burstId,'apple:U-1');assert.equal(apple[0].coverPath,'b/IMG_001.JPG');assert.equal(apple[0].members.length,3);
  const huawei=await resolveMedia([file('h/IMG_20240101_101010_BURST001.jpg',jpeg()),file('h/IMG_20240101_101010_BURST002_COVER.jpg',jpeg()),file('h/IMG_20240101_101010_BURST003.jpg',jpeg())]);
  assert.equal(huawei.length,1);assert.equal(huawei[0].coverPath,'h/IMG_20240101_101010_BURST002_COVER.jpg');
  const pixel=await resolveMedia([file('p/00000IMG_00000_BURST20190830124155263_COVER.jpg',jpeg()),file('p/00001IMG_00001_BURST20190830124155263.jpg',jpeg())]);
  assert.equal(pixel.length,1);assert.match(pixel[0].coverPath,/COVER/);
  const samsung=await resolveMedia([file('s/20231207_101605_001.jpg',jpeg()),file('s/20231207_101605_002.jpg',jpeg()),file('s/20231207_101700.jpg',jpeg())]);
  assert.equal(samsung.length,2);assert.equal(samsung.find(i=>i.kind==='burst').members.length,2);
  const ex=s=>({make:'Canon',model:'R6',date:`2024:01:01 10:00:${s}`});
  const timed=await resolveMedia([['00','100'],['00','400'],['00','800'],['05','000']].map(([s,ms],i)=>file(`t/c${i}.jpg`,jpeg({exif:{...ex(s),subsec:ms}}))));
  assert.equal(timed.length,2);assert.equal(timed.find(i=>i.kind==='burst').burstGrouping,'timing');
});
test('Takeout sidecars: supplemental-metadata, (1) counter quirk, truncation, -edited',async()=>{
  const ts=s=>({title:s,photoTakenTime:{timestamp:'1700000000'},geoData:{latitude:48.85,longitude:2.35,altitude:0}});
  const m=takeoutMatcher([
    {path:'T/IMG.jpg(1).json',name:'IMG.jpg(1).json',json:ts('IMG.jpg')},
    {path:'T/IMG.jpg.supplemental-metadata.json',name:'IMG.jpg.supplemental-metadata.json',json:ts('IMG.jpg')},
    {path:'T/x',name:'PXL_20230101_123456789.PORTRAIT.ORIGINAL.jpg.su.json',json:ts('PXL_20230101_123456789.PORTRAIT.ORIGINAL.jpg')},
  ]);
  assert.equal(m('IMG(1).jpg').name,'IMG.jpg(1).json');
  assert.equal(m('IMG.jpg').name,'IMG.jpg.supplemental-metadata.json');
  assert.equal(m('IMG-edited.jpg').name,'IMG.jpg.supplemental-metadata.json');
  assert.equal(m('PXL_20230101_123456789.PORTRAIT.ORIGINAL.jpg').name,'PXL_20230101_123456789.PORTRAIT.ORIGINAL.jpg.su.json');
  assert.equal(m('OTHER.jpg'),null);
  const items=await resolveMedia([file('Takeout/Google Photos/Trip/IMG(1).jpg',jpeg()),json('Takeout/Google Photos/Trip/IMG.jpg(1).json',ts('IMG.jpg'))]);
  assert.equal(items.length,1);assert.equal(items[0].takenAt,1700000000000);assert.equal(items[0].takenAtSource,'sidecar:takeout');
  assert.equal(items[0].gps.lat,48.85);assert.ok(items[0].files.some(f=>f.role==='sidecar'));
});
test('Embedded EXIF beats the Takeout sidecar; indexer value beats everything',async()=>{
  const files=[file('T/a.jpg',jpeg({exif:{date:'2020:01:01 10:00:00',offset:'+00:00'}})),json('T/a.jpg.json',{title:'a.jpg',photoTakenTime:{timestamp:'1700000000'}})];
  assert.equal((await resolveMedia(files))[0].takenAtSource,'exif');
  const [item]=await resolveMedia(files,{indexed:{'T/a.jpg':{takenAt:UTC(2019,5,1),gps:{lat:1,lon:2}}}});
  assert.equal(item.takenAtSource,'indexer');assert.equal(item.takenAt,UTC(2019,5,1));assert.deepEqual(item.gps,{lat:1,lon:2,source:'indexer'});
});
test('Instagram/Facebook export JSON with mojibake text',async()=>{
  assert.equal(fixMojibake('caf\u00c3\u00a9 \u00e2\u0080\u0099'),'café ’');
  const ig={ig_reels_media:[{media:[{uri:'media/reels/202301/clip_1.mp4',creation_timestamp:1673000000,title:'Fr\u00c3\u00bchling'}]}]};
  assert.equal(parseMetaExport(ig,{path:'instagram/your_instagram_activity/content/reels.json'})[0].title,'Frühling');
  const fb=[{attachments:[{data:[{media:{uri:'photos_and_videos/album/p_n.jpg',creation_timestamp:1600000000,media_metadata:{photo_metadata:{exif_data:[{taken_timestamp:1590000000}]}}}}]}]}];
  assert.equal(parseMetaExport(fb)[0].source,'sidecar:facebook-taken');
  const items=await resolveMedia([file('instagram-export/media/reels/202301/clip_1.mp4',movie({brand:'isom',durationSec:20,created:UTC(2024,5,5)})),json('instagram-export/your_instagram_activity/content/reels.json',ig)]);
  assert.equal(items[0].takenAt,1673000000000);assert.equal(items[0].takenAtSource,'sidecar:instagram');assert.equal(items[0].platform,'instagram');
  assert.equal(items[0].kind,'short');
});
test('Snapchat memories_history.json / .html and -main + -overlay stacking',async()=>{
  const snap={'Saved Media':[{Date:'2023-01-15 18:30:12 UTC','Media Type':'Image',Location:'Latitude, Longitude: 40.7, -74.0','Download Link':'https://app.snapchat.com/dmd/memories?uid=x&sid=ABCDEF123456&mid=11111111-2222'}]};
  assert.equal(parseSnapchatJson(snap)[0].takenAt,UTC(2023,0,15,18,30,12));
  const html='<table><tr><th>Date</th></tr><tr><td>2023-01-15 18:30:12 UTC</td><td>Image</td><td>Latitude, Longitude: 40.7, -74.0</td><td><a href="#" onclick="downloadMemories(\'https://x?mid=11111111-2222\')">dl</a></td></tr></table>';
  assert.equal(parseSnapchatHtml(html)[0].ids[0],'11111111-2222');
  const items=await resolveMedia([file('memories/2023-01-15_11111111-2222-main.jpg',jpeg()),file('memories/2023-01-15_11111111-2222-overlay.png',png({exif:{}})),json('json/memories_history.json',snap)]);
  assert.equal(items.length,1);assert.equal(items[0].takenAtSource,'sidecar:snapchat');assert.equal(items[0].takenAt,UTC(2023,0,15,18,30,12));
  assert.equal(items[0].overlayPath,'memories/2023-01-15_11111111-2222-overlay.png');assert.equal(items[0].gps.lat,40.7);
});
test('Filename dates: WhatsApp, screenshots, screen recordings, camera names',()=>{
  const wa=dateFromFilename('IMG-20230405-WA0012.jpg');assert.equal(wa.rule,'whatsapp');assert.equal(wa.date.takenAtLocal,'2023-04-05T00:00:00');assert.equal(wa.date.precision,'day');
  assert.equal(dateFromFilename('VID-20230405-WA0001.mp4').platform,'whatsapp');
  assert.equal(dateFromFilename('Screenshot_20240102-131415_Chrome.jpg').date.takenAtLocal,'2024-01-02T13:14:15');
  assert.equal(dateFromFilename('Screenshot 2024-01-02 at 1.14.15\u202fPM.png').date.takenAtLocal,'2024-01-02T13:14:15');
  assert.equal(dateFromFilename('Screen_Recording_20240102-131415_YouTube.mp4').kind,'screen-recording');
  assert.equal(dateFromFilename('RPReplay_Final1700000000.MP4').date.takenAt,1700000000000);
  assert.equal(dateFromFilename('PXL_20240102_131415123.jpg').date.takenAt,localMs('2024-01-02T13:14:15.123'));
  assert.equal(dateFromFilename('IMG-20231399-WA0001.jpg').date,null,'impossible dates are rejected');
});
test('Date priority: filename beats mtime; mtime is last resort; source recorded',async()=>{
  const [wa]=await resolveMedia([file('WhatsApp/Media/IMG-20230405-WA0012.jpg',jpeg())]);
  assert.equal(wa.takenAtSource,'filename:whatsapp');assert.equal(wa.takenAtConfidence,'filename');assert.equal(wa.platform,'whatsapp');
  const [plain]=await resolveMedia([file('random.jpg',jpeg(),UTC(2022,1,2))]);
  assert.equal(plain.takenAtSource,'mtime');assert.equal(plain.takenAt,UTC(2022,1,2));
});
test('Classification: screenshot, screen recording, slo-mo, time-lapse, short — only with evidence',async()=>{
  const items=await resolveMedia([
    file('Screenshot_20240102-131415.png',png({exif:{}})),
    file('ios/IMG_0005.PNG',png({exif:{userComment:'Screenshot'}})),
    file('Screen_Recording_20240102-131415.mp4',movie({brand:'isom',durationSec:30})),
    file('slow.MOV',movie({durationSec:10,fps:240})),
    file('tl.mp4',movie({brand:'isom',durationSec:10,fps:30,keys:{'com.android.capture.fps':2}})),
    file('long.mp4',movie({brand:'isom',durationSec:600})),
    file('nodata.mp4',Buffer.from('not really a video')),
  ]);
  const kind=p=>items.find(i=>i.primaryPath===p).kind;
  assert.equal(kind('Screenshot_20240102-131415.png'),'screenshot');
  assert.equal(kind('ios/IMG_0005.PNG'),'screenshot');
  assert.equal(kind('Screen_Recording_20240102-131415.mp4'),'screen-recording');
  assert.equal(kind('slow.MOV'),'slo-mo');
  assert.equal(kind('tl.mp4'),'time-lapse');
  assert.equal(kind('long.mp4'),'video');
  assert.equal(kind('nodata.mp4'),'video','no metadata → no guess');
});
test('Malformed and truncated files never throw',async()=>{
  const junk=[Buffer.from([0xff,0xd8,0xff,0xe1,0xff,0xff,0x45]),Buffer.from('....ftypheic'),Buffer.alloc(0),Buffer.from([0x89,0x50,0x4e,0x47,0,0,0,0])];
  const items=await resolveMedia(junk.map((b,i)=>file(`j/${i}.${['jpg','heic','jpg','png'][i]}`,b)));
  assert.equal(items.length,4);
});
test('ZIP import keeps export sidecars separately so dates can be recovered',async()=>{
  const {expandZip}=await import('../../web/archives.js');
  const {deflateRawSync}=await import('node:zlib');void deflateRawSync;
  const entries=[['Takeout/Google Photos/a.jpg',jpeg()],['Takeout/Google Photos/a.jpg.json',Buffer.from(JSON.stringify({title:'a.jpg',photoTakenTime:{timestamp:'1700000000'}}))],['notes.txt',Buffer.from('x')]];
  const locals=[],centrals=[];let offset=0;
  for(const [name,data] of entries){const n=Buffer.from(name);const l=Buffer.alloc(30);l.writeUInt32LE(0x04034b50,0);l.writeUInt32LE(data.length,18);l.writeUInt32LE(data.length,22);l.writeUInt16LE(n.length,26);
    const c=Buffer.alloc(46);c.writeUInt32LE(0x02014b50,0);c.writeUInt32LE(data.length,20);c.writeUInt32LE(data.length,24);c.writeUInt16LE(n.length,28);c.writeUInt32LE(offset,42);
    locals.push(l,n,data);centrals.push(c,n);offset+=30+n.length+data.length;}
  const dir=Buffer.concat(centrals),end=Buffer.alloc(22);end.writeUInt32LE(0x06054b50,0);end.writeUInt16LE(3,8);end.writeUInt16LE(3,10);end.writeUInt32LE(dir.length,12);end.writeUInt32LE(offset,16);
  const result=await expandZip(new File([Buffer.concat([...locals,dir,end])],'takeout.zip'));
  assert.equal(result.files.length,1);assert.equal(result.sidecars.length,1);assert.equal(result.skipped,1);
  const [item]=await resolveMedia([...result.files,...result.sidecars]);
  assert.equal(item.takenAtSource,'sidecar:takeout');
});
test('Gallery hides companion files (Live video, extra burst frames, overlays) but Trash still shows them',async()=>{
  const {visibleItems}=await import('../../web/organizer.js');
  const rows=[{id:1,name:'a.heic',tags:[],takenAt:1,mediaRole:'primary'},{id:2,name:'a.mov',tags:[],takenAt:1,mediaRole:'live-video'},{id:3,name:'b.jpg',tags:[],takenAt:1,mediaRole:'burst-frame',deletedAt:5}];
  assert.deepEqual(visibleItems(rows).map(r=>r.id),[1]);
  assert.deepEqual(visibleItems(rows,{view:'trash'}).map(r=>r.id),[3]);
});
test('capturedAt now reads HEIC, PNG and video dates, not only JPEG',async()=>{
  const {capturedAt}=await import('../../web/media.js');
  assert.equal(await capturedAt(file('a.heic',heic({exif:{date:'2021:07:08 09:10:11',offset:'+00:00'}}))),UTC(2021,6,8,9,10,11));
  assert.equal(await capturedAt(file('a.png',png({exif:{date:'2021:07:08 09:10:11',offset:'+00:00'}}))),UTC(2021,6,8,9,10,11));
  assert.equal(await capturedAt(file('a.mp4',movie({brand:'isom',created:UTC(2021,6,8)}))),UTC(2021,6,8));
});
