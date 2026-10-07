import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { Runtime } from './runtime.mjs';
import { Store } from './store.mjs';
const store = new Store(resolve('data/control-plane.sqlite'));
try {
  if (process.argv[2] === 'audit') console.log(JSON.stringify(store.verify(), null, 2));
  else {
    if (!process.argv[2]) throw new Error('Usage: npm start -- request.json [policy.json]; npm run audit');
    const policy = JSON.parse(readFileSync(process.argv[3] || 'config/principals.json', 'utf8'));
    const result = new Runtime({ store, principals: policy }).execute(JSON.parse(readFileSync(process.argv[2], 'utf8')));
    console.log(JSON.stringify(result, null, 2)); process.exitCode = result.status === 'COMPLETED' ? 0 : 1;
  }
} finally { store.close(); }
