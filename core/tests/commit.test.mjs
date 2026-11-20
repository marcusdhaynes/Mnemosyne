import test from 'node:test';
import assert from 'node:assert/strict';
import {DEFAULT_MODE,dryRun,applyAlbumPlan,undoAlbumPlan} from '../../web/commit.js';

const photo=(id)=>({id,name:`${id}.jpg`,albumIds:[]});

test('default commit mode is album_only',()=>{
  assert.equal(DEFAULT_MODE,'album_only');
});

test('album-only commit adds membership and undo removes it without touching files',()=>{
  const catalog={photos:[photo('p1'),photo('p2')],albums:[]};
  const plan={mode:'album_only',ops:[{op:'album_add',item_id:'p1',album_id:'a1',album_title:'Trip'}]};
  const applied=applyAlbumPlan(plan,catalog);
  assert.deepEqual(applied.catalog.photos.find(p=>p.id==='p1').albumIds,['a1']);
  assert.equal(applied.catalog.photos.find(p=>p.id==='p2').albumIds.length,0);
  assert.equal(applied.catalog.albums[0].id,'a1');
  const undone=undoAlbumPlan(applied.journal,applied.catalog);
  assert.deepEqual(undone.catalog.photos.find(p=>p.id==='p1').albumIds,[]);
  assert.equal(undone.journal.undone,true);
});

test('album-only plans reject file moves, and move requires confirmation',()=>{
  const bad=dryRun({mode:'album_only',ops:[{op:'move',from:'a.jpg',to:'b.jpg'}]});
  assert.equal(bad.ok,false);
  const move=dryRun({mode:'move',confirmed:false,ops:[{op:'move',from:'a.jpg',to:'b.jpg'}]});
  assert.equal(move.ok,false);
  assert.match(move.confirmationNeeded,/confirmed/);
});
