//! Durable, bounded-memory metadata indexing for large collections.
//! This module never opens, moves or deletes the original image bytes.
use rusqlite::{params, Connection};
use serde::Serialize;
use std::{fs, path::Path, time::{Instant,UNIX_EPOCH}};
use walkdir::WalkDir;

#[derive(Debug,Serialize)]
pub struct CatalogReport {
  pub root: String,
  pub scanned: u64,
  pub changed: u64,
  pub skipped: u64,
  pub elapsed_ms: u128,
}
#[derive(Debug,Serialize)]
pub struct CatalogEntry {
  pub path: String,
  pub name: String,
  pub size: i64,
  pub modified_ms: i64,
  pub kind: String,
  pub root: String,
}
#[derive(Debug,Serialize)]
pub struct CatalogPage { pub total: i64, pub offset: u64, pub next_offset: Option<u64>, pub files: Vec<CatalogEntry> }

fn kind(path: &Path)->Option<&'static str>{
  match path.extension()?.to_str()?.to_ascii_lowercase().as_str(){
    "jpg"|"jpeg"|"png"|"webp"|"gif"|"bmp"|"tif"|"tiff"|"avif"|"heic"|"heif"|"jxl"|"dng"|"nef"|"cr2"|"cr3"|"arw"|"raf"|"orf"|"rw2"|"srw"|"pef"|"x3f"|"3fr"|"iiq"|"erf"|"mef"|"mrw"|"kdc"|"rwl"|"mos" => Some("image"),
    "mp4"|"m4v"|"mov"|"webm"|"avi"|"mkv"|"3gp"|"3g2"|"mts"|"m2ts"|"mpeg"|"mpg"|"wmv"|"flv"|"f4v"|"vob"|"ogv"|"ts"|"asf"|"hevc"|"mxf" => Some("video"),
    _=>None,
  }
}
fn connect(db: &Path)->Result<Connection,String>{
  if let Some(parent)=db.parent(){fs::create_dir_all(parent).map_err(|e|e.to_string())?;}
  let conn=Connection::open(db).map_err(|e|e.to_string())?;
  conn.busy_timeout(std::time::Duration::from_secs(8)).map_err(|e|e.to_string())?;
  conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;
    CREATE TABLE IF NOT EXISTS indexed_media(
      path TEXT PRIMARY KEY,
      root TEXT NOT NULL,
      name TEXT NOT NULL,
      size INTEGER NOT NULL,
      modified_ms INTEGER NOT NULL,
      kind TEXT NOT NULL,
      indexed_ms INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_media_root_time ON indexed_media(root, modified_ms DESC, path ASC);
    CREATE INDEX IF NOT EXISTS idx_media_kind ON indexed_media(root, kind);"
  ).map_err(|e|e.to_string())?;
  Ok(conn)
}
pub fn index_folder(folder:&str, db:&Path)->Result<CatalogReport,String>{
  let root=fs::canonicalize(folder).map_err(|e|format!("Cannot open media folder: {e}"))?;
  if !root.is_dir(){return Err("Selected source is not a folder".into());}
  let root_text=root.to_string_lossy().to_string();
  let mut conn=connect(db)?;let started=Instant::now();
  let mut scanned=0u64;let mut changed=0u64;let mut skipped=0u64;
  // Streaming traversal + a single SQLite transaction: O(1) Rust memory.
  let tx=conn.transaction().map_err(|e|e.to_string())?;
  {
    let mut insert=tx.prepare_cached("INSERT INTO indexed_media(path,root,name,size,modified_ms,kind,indexed_ms)
      VALUES (?1,?2,?3,?4,?5,?6,?7)
      ON CONFLICT(path) DO UPDATE SET root=excluded.root,name=excluded.name,size=excluded.size,
        modified_ms=excluded.modified_ms,kind=excluded.kind,indexed_ms=excluded.indexed_ms
      WHERE indexed_media.size<>excluded.size OR indexed_media.modified_ms<>excluded.modified_ms OR indexed_media.root<>excluded.root")
      .map_err(|e|e.to_string())?;
    for entry in WalkDir::new(&root).follow_links(false).into_iter(){
      let item=match entry{Ok(v)=>v,Err(_)=>{skipped+=1;continue}};
      if !item.file_type().is_file(){continue;}
      let extension_kind=match kind(item.path()){Some(v)=>v,None=>continue};
      let meta=match item.metadata(){Ok(v)=>v,Err(_)=>{skipped+=1;continue}};
      let modified_ms=meta.modified().ok().and_then(|time|time.duration_since(UNIX_EPOCH).ok()).map(|d|d.as_millis().min(i64::MAX as u128) as i64).unwrap_or(0);
      let now=std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis().min(i64::MAX as u128) as i64;
      let path=item.path().to_string_lossy();let name=item.file_name().to_string_lossy();
      scanned+=1;
      match insert.execute(params![path.as_ref(),root_text,name.as_ref(),meta.len().min(i64::MAX as u64) as i64,modified_ms,extension_kind,now]){
        Ok(num)=>changed+=num as u64,Err(_)=>skipped+=1,
      }
    }
  }
  tx.commit().map_err(|e|e.to_string())?;
  Ok(CatalogReport{root:root_text,scanned,changed,skipped,elapsed_ms:started.elapsed().as_millis()})
}
pub fn page(folder:&str,db:&Path,offset:u64,limit:u32)->Result<CatalogPage,String>{
  let root=fs::canonicalize(folder).map_err(|e|e.to_string())?.to_string_lossy().to_string();
  let conn=connect(db)?;
  let total:i64=conn.query_row("SELECT COUNT(*) FROM indexed_media WHERE root=?1",params![root],|r|r.get(0)).map_err(|e|e.to_string())?;
  let size=limit.clamp(1,512);
  let mut stmt=conn.prepare("SELECT path,name,size,modified_ms,kind,root FROM indexed_media WHERE root=?1 ORDER BY modified_ms DESC,path ASC LIMIT ?2 OFFSET ?3").map_err(|e|e.to_string())?;
  let rows=stmt.query_map(params![root,size,i64::try_from(offset).unwrap_or(i64::MAX)],|r|Ok(CatalogEntry{
    path:r.get(0)?,name:r.get(1)?,size:r.get(2)?,modified_ms:r.get(3)?,kind:r.get(4)?,root:r.get(5)?
  })).map_err(|e|e.to_string())?;
  let mut files=Vec::with_capacity(size as usize);
  for entry in rows{files.push(entry.map_err(|e|e.to_string())?);}
  let next=offset.saturating_add(files.len() as u64);
  Ok(CatalogPage{total,offset,next_offset:if next<(total as u64){Some(next)}else{None},files})
}
#[cfg(test)]mod tests{
  use super::*;
  #[test]fn indexes_over_five_thousand_without_truncation(){
    let base=std::env::temp_dir().join(format!("mnemosyne-indexer-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    let photos=base.join("photos");fs::create_dir_all(&photos).unwrap();
    for i in 0..5050 {fs::write(photos.join(format!("{i:05}.jpg")),[]).unwrap();}
    fs::write(photos.join("note.txt"),b"ignored").unwrap();
    let db=base.join("catalog.db");let report=index_folder(photos.to_str().unwrap(),&db).unwrap();
    assert_eq!(report.scanned,5050);assert_eq!(page(photos.to_str().unwrap(),&db,0,256).unwrap().files.len(),256);
    let tail=page(photos.to_str().unwrap(),&db,5000,256).unwrap();assert_eq!(tail.files.len(),50);assert_eq!(tail.next_offset,None);
    let again=index_folder(photos.to_str().unwrap(),&db).unwrap();assert_eq!(again.changed,0);
    let _=fs::remove_dir_all(&base);
  }
}
