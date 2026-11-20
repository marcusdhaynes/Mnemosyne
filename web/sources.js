/** A truthful registry: a card is not an OAuth integration.
 * Native USB / drive-letter discovery and journaled commit live in the
 * `photo_sources` Rust crate. Cloud rows stay stubs until OAuth is configured.
 * Export ZIP/folder unpacking enumerates media and sidecar files, then calls
 * `resolveMedia` in web/formats/index.js (Spin) — this registry does not recover sidecar dates itself.
 */
export const COMMIT_DEFAULT = 'album_only';

export const SOURCES = [
  {id:'device',name:'This Device',icon:'◈',method:'file',availability:'ready',detail:'Choose photos, videos and shorts from any accessible local device.'},
  {id:'folder',name:'Camera & Folder',icon:'▣',method:'folder',availability:'ready',detail:'Import an entire folder recursively, including DCIM, SD cards, USB drives and camera exports. On Windows, phones connected over USB (File transfer or PTP) appear as a drive letter and are scanned from here.'},
  {id:'zip',name:'Social Media Exports',icon:'▤',method:'archive',availability:'ready',detail:'Choose an exported ZIP from Instagram, TikTok, Google Takeout, Facebook or Snapchat, or import the extracted folder. Sidecar JSON is passed to the format hook; it is not parsed here.'},
  {id:'google-drive',name:'Google Drive',icon:'◉',method:'oauth-picker',availability:'setup',detail:'Requires an approved Google OAuth client, Drive API and Picker configuration. Use a synced Drive folder or a Takeout export meanwhile.',docs:'https://developers.google.com/workspace/drive/picker/guides/web-picker'},
  {id:'google-photos',name:'Google Photos',icon:'◍',method:'oauth-picker',availability:'setup',detail:'Since March 2025, Google Photos only permits user-selected assets (Photos Picker) or a Google Takeout of the library; unrestricted third-party library scanning is not available.',docs:'https://developers.google.com/photos/overview/about'},
  {id:'apple-photos',name:'Apple Photos / iCloud',icon:'✧',method:'native',availability:'native',detail:'iCloud has no public photos API. On Windows, add the iCloud for Windows synced folder (typically iCloudPhotos\\\\Photos) or import an export. Native PhotoKit is a future macOS/iOS route.',docs:'https://developer.apple.com/documentation/photokit/'},
  {id:'onedrive',name:'OneDrive',icon:'◇',method:'oauth-picker',availability:'setup',detail:'Microsoft Graph OAuth integration requires app registration and approved Files permissions. Until then, add a synced OneDrive folder.',docs:'https://learn.microsoft.com/graph/api/resources/onedrive'},
  {id:'dropbox',name:'Dropbox',icon:'⬡',method:'oauth-picker',availability:'setup',detail:'Dropbox OAuth and file chooser require an app key, redirect domains and permissions. Until then, add a synced Dropbox folder.',docs:'https://www.dropbox.com/developers/chooser'},
  {id:'android',name:'Android Gallery',icon:'▦',method:'native',availability:'native',detail:'Prefer USB File transfer so the phone mounts as a drive and DCIM can be imported as a folder. Direct MTP (Windows WPD) is not implemented. Native Android Photo Picker requires an Android build.'},
  {id:'nas',name:'External Disks & NAS',icon:'▥',method:'folder',availability:'ready',detail:'Use folder import for mounted network and external drives; automatic monitoring requires the desktop edition.'}
];
