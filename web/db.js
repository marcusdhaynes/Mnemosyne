const DB_NAME='mnemosyne-library';const DB_VERSION=1;
let dbPromise;
export function database(){
  if(!dbPromise)dbPromise=new Promise((resolve,reject)=>{
    if(!('indexedDB' in window))return reject(new Error('IndexedDB unavailable in this browser'));
    const request=indexedDB.open(DB_NAME,DB_VERSION);
    request.onupgradeneeded=()=>{const db=request.result;
      if(!db.objectStoreNames.contains('photos')){const p=db.createObjectStore('photos',{keyPath:'id'});p.createIndex('hash','hash',{unique:false});p.createIndex('takenAt','takenAt',{unique:false});}
      if(!db.objectStoreNames.contains('media'))db.createObjectStore('media',{keyPath:'id'});
      if(!db.objectStoreNames.contains('albums'))db.createObjectStore('albums',{keyPath:'id'});
    };
    request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);
  });return dbPromise;
}
export async function all(store){const db=await database();return new Promise((resolve,reject)=>{const r=db.transaction(store,'readonly').objectStore(store).getAll();r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error);});}
export async function get(store,key){const db=await database();return new Promise((resolve,reject)=>{const r=db.transaction(store,'readonly').objectStore(store).get(key);r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error);});}
export async function put(store,item){const db=await database();return new Promise((resolve,reject)=>{const tx=db.transaction(store,'readwrite');tx.objectStore(store).put(item);tx.oncomplete=()=>resolve(item);tx.onerror=()=>reject(tx.error);});}
export async function remove(store,key){const db=await database();return new Promise((resolve,reject)=>{const tx=db.transaction(store,'readwrite');tx.objectStore(store).delete(key);tx.oncomplete=()=>resolve();tx.onerror=()=>reject(tx.error);});}
export async function savePhoto(photo,original,thumbnail){const db=await database();return new Promise((resolve,reject)=>{const tx=db.transaction(['photos','media'],'readwrite');tx.objectStore('photos').put(photo);tx.objectStore('media').put({id:photo.id,original,thumbnail});tx.oncomplete=()=>resolve(photo);tx.onerror=()=>reject(tx.error);});}
export async function destroyPhoto(id){const db=await database();return new Promise((resolve,reject)=>{const tx=db.transaction(['photos','media'],'readwrite');tx.objectStore('photos').delete(id);tx.objectStore('media').delete(id);tx.oncomplete=()=>resolve();tx.onerror=()=>reject(tx.error);});}

/** One transaction per page rather than hundreds of sequential transactions. */
export async function mediaBatch(ids){
  const db=await database();if(!ids.length)return [];
  return new Promise((resolve,reject)=>{
    const tx=db.transaction('media','readonly'),store=tx.objectStore('media'),result=new Array(ids.length);
    ids.forEach((id,i)=>{const req=store.get(id);req.onsuccess=()=>{result[i]=req.result;};});
    tx.oncomplete=()=>resolve(result);tx.onerror=()=>reject(tx.error);
  });
}
export async function savePhotoBatch(entries){
  if(!entries.length)return;
  const db=await database();return new Promise((resolve,reject)=>{
    const tx=db.transaction(['photos','media'],'readwrite'),p=tx.objectStore('photos'),m=tx.objectStore('media');
    for(const entry of entries){p.put(entry.photo);m.put({id:entry.photo.id,original:entry.original,thumbnail:entry.thumbnail});}
    tx.oncomplete=()=>resolve();tx.onerror=()=>reject(tx.error);tx.onabort=()=>reject(tx.error||new Error('Import transaction aborted'));
  });
}
export async function putPhotoBatch(photos){
  if(!photos.length)return;
  const db=await database();return new Promise((resolve,reject)=>{
    const tx=db.transaction('photos','readwrite'),store=tx.objectStore('photos');
    for(const p of photos)store.put(p);
    tx.oncomplete=()=>resolve();tx.onerror=()=>reject(tx.error);
  });
}
