import {createServer} from 'node:http';
import {readFile,stat} from 'node:fs/promises';
import {resolve,extname,join,sep} from 'node:path';
const dir=resolve(process.argv[2]||'web');const port=Number(process.env.PORT||5173);
const types={'.html':'text/html; charset=utf-8','.css':'text/css; charset=utf-8','.js':'text/javascript; charset=utf-8','.svg':'image/svg+xml','.png':'image/png','.ico':'image/x-icon','.json':'application/json'};
createServer(async(req,res)=>{try{const url=new URL(req.url||'/',`http://${req.headers.host||'localhost'}`);const path=resolve(dir,`.${decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname)}`);if(path!==dir&&!path.startsWith(dir+sep)){res.writeHead(403).end();return;}if(!(await stat(path)).isFile())throw new Error('Not a file');const content=await readFile(path);res.writeHead(200,{'Content-Type':types[extname(path)]||'application/octet-stream','Cache-Control':'no-cache','X-Content-Type-Options':'nosniff'}).end(content);}catch{res.writeHead(404,{'Content-Type':'text/plain'}).end('Not found');}}).listen(port,'127.0.0.1',()=>console.log(`Mnemosyne running at http://localhost:${port}/`));
