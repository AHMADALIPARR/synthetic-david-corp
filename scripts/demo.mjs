// Explicit nonfinancial development fixture. No banking transaction is created.
import { randomUUID } from 'node:crypto';
import { Runtime } from '../src/runtime.mjs';
import { Store } from '../src/store.mjs';
import { hash } from '../src/contracts.mjs';
const store = new Store();
try {
  const text = 'Synthetic David development fixture.\nDocument inspection only.';
  const runtime = new Runtime({store,principals:{'demo-reviewer':{agents:['DOCUMENT'],permissions:['document:read']}}});
  const result = runtime.execute({requestId:randomUUID(),traceId:randomUUID(),graphId:randomUUID(),requestor:'demo-reviewer',type:'DOCUMENT-INSPECT',payload:{
    source:{id:'DEVELOPMENT-FIXTURE',location:'scripts/demo.mjs',sha256:hash(text)},text
  }});
  console.log(JSON.stringify(result,null,2));
  if(result.status !== 'COMPLETED') process.exitCode=1;
} finally {store.close();}
