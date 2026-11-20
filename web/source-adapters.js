/** Source adapters for the web path: locate exports and list media + sidecars.
 * No sidecar parsing or grouping here; pass the files to Spin's `resolveMedia`
 * in `web/formats/index.js`, which owns dates, Live/Motion Photos and bursts.
 */
import {mediaKind} from './organizer.js';

export const COMMIT_DEFAULT = 'album_only';

export function isSidecarName(name=''){
  const lower=String(name).toLowerCase().split(/[\\/]/).pop()||'';
  return lower.endsWith('.supplemental-metadata.json')
    || lower.endsWith('.supplemental-meta.json')
    || lower.endsWith('.json')
    || lower.endsWith('.xmp')
    || lower.endsWith('.aae')
    || (lower.endsWith('.html') && lower.includes('memories'));
}

export function virtualPathOf(file){
  return file.virtualPath||file.webkitRelativePath||file.name||'';
}

export function detectExportKind(paths){
  const text=paths.map(p=>String(p).replaceAll('\\','/').toLowerCase()).join('\n');
  const score={takeout:0,instagram:0,facebook:0,snapchat:0,tiktok:0};
  if(/takeout\/|google photos\/|supplemental-metadata\.json|\.jpg\.json/.test(text))score.takeout+=5;
  if(/instagram|your_instagram_activity|posts_1\.json|\/media\/posts/.test(text))score.instagram+=5;
  if(/facebook|photos_and_videos|your_posts\.json|your_facebook_activity/.test(text))score.facebook+=5;
  if(/snapchat|memories_history|\/memories\//.test(text))score.snapchat+=5;
  if(/tiktok|user_data\.json/.test(text))score.tiktok+=5;
  const best=Object.entries(score).sort((a,b)=>b[1]-a[1])[0];
  return best[1]>=5?best[0]:null;
}

function dirOf(path){
  const norm=String(path).replaceAll('\\','/');
  const i=norm.lastIndexOf('/');
  return i===-1?'':norm.slice(0,i);
}

function basename(path){
  return String(path).replaceAll('\\','/').split('/').pop()||'';
}

function sidecarFor(mediaPath, sidecarPaths){
  const name=basename(mediaPath);
  const stem=name.replace(/\.[^.]+$/,'');
  const parent=dirOf(mediaPath);
  const suffixes=[
    `${name}.json`,`${name}.supplemental-metadata.json`,`${name}.supplemental-meta.json`,
    `${stem}.json`,`${stem}.xmp`,`${name}.xmp`,`${stem}.aae`
  ];
  return sidecarPaths.filter(p=>dirOf(p)===parent && suffixes.some(s=>basename(p).toLowerCase()===s.toLowerCase()));
}

/** List media and sidecar records with export kind. Sidecar bodies are not read. */
export function enumerateImportFiles(files,{sidecars=[]}={}){
  const all=[...files,...sidecars];
  const paths=all.map(virtualPathOf);
  const exportKind=detectExportKind(paths);
  const sidecarFiles=all.filter(f=>isSidecarName(virtualPathOf(f)||f.name));
  const sidecarPaths=sidecarFiles.map(virtualPathOf);
  const media=files.filter(f=>mediaKind(f)||mediaKind({name:virtualPathOf(f),type:f.type||''}));
  const records=media.map(file=>{
    const path=virtualPathOf(file);
    return {
      file,
      name:file.name,
      path,
      role:'media',
      mediaKind:mediaKind(file)||mediaKind({name:path,type:file.type||''}),
      exportKind,
      sidecarPaths:sidecarFor(path,sidecarPaths),
      bytes:file.size||0,
      format:null
    };
  });
  const sidecarRecords=sidecarFiles.map(file=>({
    file,name:file.name,path:virtualPathOf(file),role:'sidecar',mediaKind:null,
    exportKind,sidecarPaths:[],bytes:file.size||0,format:null
  }));
  return [...records,...sidecarRecords];
}

export const CLOUD_STUBS=[
  {id:'google-drive',name:'Google Drive',route:'Requires an approved OAuth client and the Google Drive API / Picker (drive.file). This build makes no network calls. Until configured, add a synced Drive folder or import a Google Takeout zip.'},
  {id:'google-photos',name:'Google Photos',route:'As of March 2025, Google Photos only lets a third-party app pick items one by one. A full library has to come in through Google Takeout. This build makes no network calls.'},
  {id:'onedrive',name:'OneDrive',route:'Requires a Microsoft Graph app registration. This build makes no network calls. Until configured, add a synced OneDrive folder as a folder source.'},
  {id:'dropbox',name:'Dropbox',route:'Requires a Dropbox app key and the Chooser or Files API. This build makes no network calls. Until configured, add a synced Dropbox folder as a folder source.'},
  {id:'apple-photos',name:'Apple Photos / iCloud',route:'iCloud has no public photos API. On Windows, add the iCloud for Windows synced folder (typically %USERPROFILE%\\\\iCloudPhotos\\\\Photos). Otherwise import an iCloud/Photos export. Native PhotoKit is a future macOS/iOS route.'}
];

export function connectCloud(id){
  const stub=CLOUD_STUBS.find(s=>s.id===id);
  if(!stub)throw new Error(`Unknown cloud source: ${id}`);
  const error=new Error(`${stub.name} is not connected in this build. ${stub.route}`);
  error.code='STUB';
  throw error;
}
