import {cp,mkdir,rm,stat,writeFile} from 'node:fs/promises';
const src=new URL('../web/',import.meta.url),dest=new URL('../dist/',import.meta.url);
await stat(new URL('main.js',src));await rm(dest,{recursive:true,force:true});await mkdir(dest,{recursive:true});await cp(src,dest,{recursive:true});await writeFile(new URL('.nojekyll',dest),'');
console.log('Built Mnemosyne static web application in dist/');
