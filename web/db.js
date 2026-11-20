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
