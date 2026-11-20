import test from 'node:test';
import assert from 'node:assert/strict';
import {faceDetectorAvailable,makeFaceReview} from '../../web/recognition.js';
test('face detection reports availability truthfully',()=>{assert.equal(faceDetectorAvailable({}),false);assert.equal(faceDetectorAvailable({FaceDetector:class{}}),true);});
test('face detection never names people without their consent',()=>{const photo={name:'portrait.jpg',people:[],tags:[],reviewSuggestions:[]};const out=makeFaceReview(photo,[{x:1,y:1,width:10,height:10}]);assert.deepEqual(out.people,[]);assert.deepEqual(out.tags,[]);assert.ok(out.reviewSuggestions.some(s=>s.label==='Portraits'));});
