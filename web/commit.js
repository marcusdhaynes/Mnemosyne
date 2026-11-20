/** Album-first commit for the web catalog.
 * Default mode is album_only. File move/copy/rename is implemented in the
 * native `photo_sources` crate and is opt-in with confirmed=true and undo.
 * This module never moves or deletes original files.
 */
export const DEFAULT_MODE='album_only';

function nowMs(){return Date.now();}
function newId(){return 'c_'+nowMs().toString(16);}

export function dryRun(plan){
  const conflicts=[];
  const mode=plan.mode||DEFAULT_MODE;
  const confirmationNeeded=(mode==='move'||mode==='rename_only')&&!plan.confirmed
    ?'move/rename requires confirmed=true':null;
  if(mode==='album_only' && (plan.ops||[]).some(op=>op.op!=='album_add')){
    conflicts.push({message:'album_only plans may only contain album_add operations'});
  }
  for(const op of plan.ops||[]){
    if(op.op==='album_add' && (!op.item_id||!op.album_id)){
      conflicts.push({message:'album_add needs item_id and album_id'});
    }
  }
  return {ok:!conflicts.length&&!confirmationNeeded,conflicts,confirmationNeeded,mode};
}

export function applyAlbumPlan(plan, catalog){
  const report=dryRun(plan);
  if(report.confirmationNeeded)throw new Error(report.confirmationNeeded);
  if(!report.ok)throw new Error(report.conflicts.map(c=>c.message).join('; '));
  const photos=catalog.photos.map(p=>({...p,albumIds:[...(p.albumIds||[])]}));
  let albums=[...catalog.albums];
  const journal={commit_id:plan.id||newId(),mode:plan.mode||DEFAULT_MODE,started_ms:nowMs(),ops:[],undone:false};
  for(const op of plan.ops||[]){
    if(op.op!=='album_add')throw new Error('web applyAlbumPlan only supports album_add');
    if(!albums.some(a=>a.id===op.album_id))albums=[...albums,{id:op.album_id,name:op.album_title||'Album'}];
    const photo=photos.find(p=>p.id===op.item_id);
    if(photo && !photo.albumIds.includes(op.album_id))photo.albumIds.push(op.album_id);
    journal.ops.push({...op,done:true});
  }
  journal.finished_ms=nowMs();
  return {catalog:{...catalog,photos,albums},journal};
}

export function undoAlbumPlan(journal, catalog){
  if(journal.undone)throw new Error('this commit has already been undone');
  const photos=catalog.photos.map(p=>({...p,albumIds:[...(p.albumIds||[])]}));
  for(const op of [...journal.ops].reverse()){
    if(op.op==='album_add'){
      const photo=photos.find(p=>p.id===op.item_id);
      if(photo)photo.albumIds=photo.albumIds.filter(id=>id!==op.album_id);
    }
  }
  return {catalog:{...catalog,photos,albums:catalog.albums},journal:{...journal,undone:true,finished_ms:nowMs()}};
}
