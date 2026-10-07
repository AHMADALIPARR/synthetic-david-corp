import { spawn } from 'node:child_process';
import { mkdirSync, openSync, closeSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));
async function health(){const response=await fetch('http://127.0.0.1:1235/health',{signal:AbortSignal.timeout(2000)});const info=await response.json();if(!response.ok||info.service!=='david-qwen-endpoint')throw new Error('Unexpected adapter');return info;}
if(process.argv[2]==='status')console.log(JSON.stringify(await health(),null,2));
else if(process.argv[2]==='start'){
  let existing;try{existing=await health();}catch{}
  if(existing)console.log(JSON.stringify(existing,null,2));
  else{
    mkdirSync(path.join(root,'data'),{recursive:true});
    const out=openSync(path.join(root,'data/qwen-adapter.log'),'a'),err=openSync(path.join(root,'data/qwen-adapter-error.log'),'a');
    const child=spawn(path.join(root,'rust/target/debug/david-qwen-endpoint'+(process.platform==='win32'?'.exe':'')),[],{cwd:root,windowsHide:true,detached:true,stdio:['ignore',out,err]});
    child.on('error',()=>{console.error('Build the Rust adapter before starting it.');process.exitCode=1;});child.unref();closeSync(out);closeSync(err);
    let ready;for(let i=0;i<10;i++){try{ready=await health();break;}catch{}await new Promise(r=>setTimeout(r,500));}
    if(!ready)throw new Error('Rust adapter failed to start; inspect data/qwen-adapter-error.log.');
    console.log(JSON.stringify(ready,null,2));
  }
}else throw new Error('Use start or status.');
