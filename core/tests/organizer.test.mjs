import test from 'node:test';import assert from 'node:assert/strict';
import {mediaKind,monthKey,suggestTags,duplicateSets,visibleItems,similarityScore} from '../../web/organizer.js';
test('classifies supported image and video formats',()=>{assert.equal(mediaKind({name:'image.DNG',type:''}),'image');assert.equal(mediaKind({name:'clip.MOV',type:''}),'video');assert.equal(mediaKind({name:'notes.txt',type:''}),null);});
test('suggestions are filename-based, not pretend AI output',()=>{assert.deepEqual(suggestTags('family_Birthday_party.jpg'),['Celebrations','Portraits']);});
test('duplicate detection identifies exact hashes only',()=>{const p=[{id:1,hash:'aaa'},{id:2,hash:'aaa'},{id:3,hash:'bbb'},{id:4,hash:null}];assert.equal(duplicateSets(p).length,1);});
test('favorites and trash remain isolated',()=>{const p=[{id:1,hash:null,name:'cat',tags:[],favorite:true,takenAt:10},{id:2,name:'dog',tags:[],favorite:true,deletedAt:1,takenAt:20}];assert.deepEqual(visibleItems(p,{view:'favorites'}).map(p=>p.id),[1]);assert.deepEqual(visibleItems(p,{view:'trash'}).map(p=>p.id),[2]);});
test('months and visual difference',()=>{assert.equal(monthKey(new Date('2024-03-01T12:00:00Z').valueOf()),'2024-03');assert.equal(similarityScore('0101','0111'),1);});
