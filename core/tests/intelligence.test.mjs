import test from 'node:test';
import assert from 'node:assert/strict';
import {proposeOrganization,applyAutomatic,applyReviewLabels,dismissReviewLabels,isShortVideo,sourceOf,reviewSummary} from '../../web/intelligence.js';
import {normalizedEdits,photoFilter} from '../../web/editor.js';

const photo=(overrides={})=>({name:'IMG_0001.jpg',folder:'',kind:'image',size:90,takenAt:Date.UTC(2026,0,2),tags:[],...overrides});

test('verified metadata groups automatically and uncertain scene words queue for review',()=>{
  const input=photo({name:'birthday.jpg',folder:'Instagram/exports'});
  const plan=proposeOrganization(input);
  assert.equal(plan.source,'Instagram');
  assert.ok(plan.automatic.some(s=>s.label==='2026'));
  assert.ok(plan.automatic.some(s=>s.label==='Instagram'));
  assert.ok(plan.review.some(s=>s.label==='Celebrations'));
  assert.ok(!plan.automatic.some(s=>s.label==='Celebrations'));
  const result=applyAutomatic(input,plan);
  assert.ok(result.tags.includes('Instagram'));
  assert.ok(!result.tags.includes('Celebrations'));
  assert.ok(result.reviewSuggestions.some(s=>s.label==='Celebrations'));
  const approved=applyReviewLabels(result,['Celebrations']);
  assert.ok(approved.tags.includes('Celebrations'));
  assert.equal(approved.reviewSuggestions.length,0);
});
test('dismissal never touches originals or other suggestions',()=>{
  const input=applyAutomatic(photo({name:'wedding sunset.jpg',size:1000}));
  const changed=dismissReviewLabels(input,['Nature']);
  assert.equal(changed.size,1000);
  assert.ok(changed.reviewSuggestions.some(x=>x.label==='Celebrations'));
  assert.ok(!changed.reviewSuggestions.some(x=>x.label==='Nature'));
});
test('all orientations count as Shorts and unknown duration is not silently verified',()=>{
  assert.ok(isShortVideo(photo({kind:'video',duration:130,width:1920,height:1080})));
  assert.ok(isShortVideo(photo({kind:'video',duration:180,width:1080,height:1920})));
  assert.ok(!isShortVideo(photo({kind:'video',duration:181})));
  const uncertain=proposeOrganization(photo({name:'VID.mp4',kind:'video',folder:'TikTok',duration:null}));
  assert.ok(uncertain.review.some(x=>x.label==='Shorts'));
  assert.ok(!uncertain.automatic.some(x=>x.label==='Shorts'));
});
test('different export sources are recognized without mislabelling arbitrary filenames',()=>{
  assert.equal(sourceOf(photo({name:'IMG_1234.jpg'})),'This Device');
  assert.equal(sourceOf(photo({name:'clip.mov',folder:'YouTube Shorts/2026'})),'YouTube');
  assert.equal(sourceOf(photo({name:'clip.mov',folder:'Telegram Saved Messages'})),'Telegram');
  assert.equal(sourceOf(photo({name:'TikTok video.mp4'})),'TikTok');
  assert.deepEqual(reviewSummary([applyAutomatic(photo({name:'beach.jpg'})),applyAutomatic(photo())]),{media:1,suggestions:1});
});
test('model predictions never directly assign a person identity',()=>{
  const plan=proposeOrganization(photo(),{modelPredictions:[{label:'Person: Alex',confidence:0.999}]});
  assert.ok(plan.review.some(s=>s.label==='Person: Alex'));
  assert.ok(!plan.automatic.some(s=>s.label==='Person: Alex'));
});
test('non destructive edit values have safe bounds',()=>{
  assert.deepEqual(normalizedEdits({brightness:999,contrast:-5,saturation:-1,rotation:-90}),{brightness:200,contrast:20,saturation:0,rotation:270});
  assert.equal(photoFilter({contrast:150}),'brightness(100%) contrast(150%) saturate(100%)');
});
