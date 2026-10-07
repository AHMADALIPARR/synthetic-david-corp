import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../', import.meta.url));
const localState = path.join(root, '.codex-local');
const adapterHealth = await fetch('http://127.0.0.1:1235/health', {signal:AbortSignal.timeout(3000)}).then(r=>r.json());
if(adapterHealth.service !== 'david-qwen-endpoint') throw new Error('Expected the David Rust Qwen adapter.');
mkdirSync(localState,{recursive:true});
const instructions = path.join(root,'config/qwen-instructions.md').replaceAll('\\','/');
const catalog = path.join(root,'config/qwen-models.json').replaceAll('\\','/');
const config = 'model_instructions_file = ' + JSON.stringify(instructions) + '\nmodel_catalog_json = ' + JSON.stringify(catalog) + '\n' + readFileSync(path.join(root,'config/codex-qwen.toml'),'utf8');
writeFileSync(path.join(localState,'config.toml'),config);
let executable = process.env.CODEX_CLI_BIN;
let prefix = [];
if(!executable && process.platform === 'win32') {
  const installed = path.join(process.env.LOCALAPPDATA || '', 'JetBrains/DataGrip2026.2/acp-agents/codex-acp');
  const wrappers = existsSync(installed) ? readdirSync(installed).sort((a,b)=>b.localeCompare(a,undefined,{numeric:true})).map(version=>path.join(installed,version,'node_modules/@openai/codex/bin/codex.js')) : [];
  const wrapper = wrappers.find(existsSync);
  if(!wrapper) throw new Error('Set CODEX_CLI_BIN to an installed native Codex binary.');
  executable=process.execPath;prefix=[wrapper];
}
executable ||= 'codex';
const prompt = process.argv.slice(2).join(' ');
const args = prompt ? ['exec','--ephemeral','--json','-C',root,'--sandbox','read-only',prompt] : ['-C',root,'--sandbox','read-only'];
// Isolate this child only; the user's Codex home, credentials, hooks and IDE session are not rewritten.
const child = spawn(executable,[...prefix,...args], {cwd:root,windowsHide:true,stdio:prompt?['ignore','inherit','inherit']:'inherit',env:{...process.env,CODEX_HOME:localState}});
child.on('error',()=>{console.error('Codex process could not start.');process.exitCode=1;});
child.on('exit',code=>{process.exitCode=code ?? 1;});
