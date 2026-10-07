import { execFileSync } from 'node:child_process';
const entries = execFileSync('git',['ls-files','--stage','-z'],{encoding:'utf8'}).split('\0').filter(Boolean);
let count=0,bytes=0;
for(const entry of entries){
  const split=entry.indexOf('\t'), metadata=entry.slice(0,split), file=entry.slice(split+1);
  if(metadata.startsWith('160000 ')){
    if(file!=='vendor/codex')throw new Error('Unexpected submodule: '+file);
    continue;
  }
  if(/(^|\/)(\.env(?:\..*)?|\.tools|\.codex-local|node_modules|target|data|build)(\/|$)|\.(?:log|db|sqlite.*|exe|dll|gguf|bin)$/i.test(file))throw new Error('Local-only file staged: '+file);
  const content=execFileSync('git',['show',':'+file],{encoding:'utf8',maxBuffer:10*1024*1024});
  if(/\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,}|sk-(?:proj-|ant-)?[A-Za-z0-9_-]{40,})|-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/.test(content))throw new Error('Possible credential staged: '+file);
  if(content.includes('\0'))throw new Error('Binary file staged: '+file);
  count++;bytes+=Buffer.byteLength(content);
}
console.log(JSON.stringify({checkedFiles:count,totalBytes:bytes,pinnedSubmodule:'vendor/codex',localArtifactsExcluded:true}));
