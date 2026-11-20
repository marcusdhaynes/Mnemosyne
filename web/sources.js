/** A truthful registry: a card is not an OAuth integration. */
export const SOURCES = [
  {id:'device',name:'This Device',icon:'◈',method:'file',availability:'ready',detail:'Choose photos, videos and shorts from any accessible local device.'},
  {id:'folder',name:'Camera & Folder',icon:'▣',method:'folder',availability:'ready',detail:'Import an entire folder recursively, including DCIM, SD cards, USB drives and camera exports.'},
  {id:'zip',name:'Social Media Exports',icon:'▤',method:'archive',availability:'ready',detail:'Choose an exported ZIP from Instagram, TikTok, Google Takeout or another service, or import the extracted folder.'},
  {id:'google-drive',name:'Google Drive',icon:'◉',method:'oauth-picker',availability:'setup',detail:'Requires an approved Google OAuth client, Drive API and Picker configuration. Use exported files meanwhile.',docs:'https://developers.google.com/workspace/drive/picker/guides/web-picker'},
  {id:'google-photos',name:'Google Photos',icon:'◍',method:'oauth-picker',availability:'setup',detail:'Google Photos only permits user-selected assets or supported exports; unrestricted third-party library scanning is not available.',docs:'https://developers.google.com/photos/overview/about'},
  {id:'apple-photos',name:'Apple Photos / iCloud',icon:'✧',method:'native',availability:'native',detail:'Native iOS and macOS PhotoKit authorization will access approved local and iCloud-managed photos. Web: select exported files.',docs:'https://developer.apple.com/documentation/photokit/'},
  {id:'onedrive',name:'OneDrive',icon:'◇',method:'oauth-picker',availability:'setup',detail:'Microsoft Graph OAuth integration requires app registration and approved Files permissions.',docs:'https://learn.microsoft.com/graph/api/resources/onedrive'},
  {id:'dropbox',name:'Dropbox',icon:'⬡',method:'oauth-picker',availability:'setup',detail:'Dropbox OAuth and file chooser require an app key, redirect domains and permissions.',docs:'https://www.dropbox.com/developers/chooser'},
  {id:'android',name:'Android Gallery',icon:'▦',method:'native',availability:'native',detail:'Native Android Photo Picker and MediaStore require OS permissions and Android builds.'},
  {id:'nas',name:'External Disks & NAS',icon:'▥',method:'folder',availability:'ready',detail:'Use folder import for mounted network and external drives; automatic monitoring requires the desktop edition.'}
];
