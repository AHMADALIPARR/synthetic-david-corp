import test from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Runtime, selectMinimum } from '../src/runtime.mjs';
import { Store } from '../src/store.mjs';
import { amount, hash } from '../src/contracts.mjs';
const principals = { reviewer: { agents: ['LEDGER','DOCUMENT','MIG-VALID'], permissions: ['ledger:read','document:read','migration:read'] } };
function ledger() {
  return { requestId: randomUUID(), traceId: randomUUID(), graphId: randomUUID(), requestor: 'reviewer', type: 'LEDGER-VALIDATE', payload: {
    source: { id: 'explicit-test-fixture', location: 'test/runtime.test.mjs', sha256: hash('test-fixture') },
    journalId: randomUUID(), currency: 'USD', entries: [
      {entryId:randomUUID(),accountId:randomUUID(),debit:'900719925474099.1234',credit:'0.0000'},
      {entryId:randomUUID(),accountId:randomUUID(),debit:'0.0000',credit:'900719925474099.1234'}
    ] } };
}
function runtime(store) { return new Runtime({store,principals}); }
test('minimum sufficient set excludes unrelated permitted agents', () => {
  assert.deepEqual(selectMinimum(['ledger.validate'],principals.reviewer.agents,principals.reviewer.permissions),['LEDGER']);
});
test('exact amounts remain precise above JS integer limit', () => {
  const r=runtime(); const result=r.execute(ledger());
  assert.equal(result.status,'COMPLETED'); assert.equal(result.output.debitUnits,'9007199254740991234');
  assert.equal(result.output.posted,false); assert.equal(result.provenance[0].agentId,'LEDGER');
  assert.equal(r.store.verify().count,2); r.store.close();
});
test('amount syntax rejects floats, exponents, signs, extra decimals and overflow', () => {
  for(const v of [1,'1e4','-1.0000','1.00001','1000000000000000.0000']) assert.throws(()=>amount(v));
});
test('unbalanced journal escalates and persistently freezes graph', () => {
  const dir=mkdtempSync(join(tmpdir(),'david-test-')), file=join(dir,'test.sqlite');
  try {
    const req=ledger(); req.payload.entries[1].credit='900719925474099.1233';
    let r=runtime(new Store(file)); const result=r.execute(req);
    assert.equal(result.status,'ESCALATED'); assert.equal(result.errorCode,'LEDGER_MISMATCH'); r.store.close();
    r=runtime(new Store(file)); const next=ledger(); next.graphId=req.graphId;
    assert.equal(r.execute(next).errorCode,'GRAPH_FROZEN'); r.store.close();
  } finally { rmSync(dir,{recursive:true,force:true}); }
});
test('duplicate identity replays without new audit; changed payload rejected', () => {
  const r=runtime(),req=ledger(),first=r.execute(req), count=r.store.verify().count;
  assert.deepEqual(r.execute(req),first); assert.equal(r.store.verify().count,count);
  req.payload.currency='EUR'; assert.equal(r.execute(req).errorCode,'REQUEST_ID_CONFLICT'); r.store.close();
});
test('permissions and unknown registry entries fail closed', () => {
  const req=ledger(); const r=new Runtime({principals:{reviewer:{agents:['LEDGER'],permissions:[]}}});
  assert.equal(r.execute(req).errorCode,'MISSING_PERMISSION'); r.store.close();
  const other=new Runtime({principals:{reviewer:{agents:['GHOST'],permissions:['ledger:read']}}});
  req.requestId=randomUUID(); assert.equal(other.execute(req).errorCode,'UNKNOWN_AGENT'); other.store.close();
});
test('unknown tools, arbitrary shell, endpoint and payment requests cannot execute', () => {
  for(const type of ['PAYMENT-ORDER','BASH','CURL','MIGRATION-APPROVE']) {
    const r=runtime(),req=ledger();req.type=type;
    assert.equal(r.execute(req).errorCode,'UNKNOWN_TOOL');r.store.close();
  }
});
test('missing evidence, duplicate entry and mixed sides are blocked', () => {
  for(const mutate of [p=>delete p.source,p=>p.entries[1].entryId=p.entries[0].entryId,p=>p.entries[0].credit='1.0000']) {
    const r=runtime(),req=ledger(); mutate(req.payload); assert.equal(r.execute(req).status,'HALTED');r.store.close();
  }
});
test('secret fields and recognizable credentials never enter audit', () => {
  for(const secret of [{apiKey:'private-value'}, {notes:'Bearer private-value'}]) {
    const r=runtime(),req=ledger(); Object.assign(req.payload,secret);
    assert.equal(r.execute(req).errorCode,'SECRET_FIELD_FORBIDDEN'); assert.equal(r.store.verify().count,0);r.store.close();
  }
});
test('missing ids and oversized payloads are rejected', () => {
  for(const mutate of [r=>r.traceId='',r=>r.payload.padding='x'.repeat(8193)]) {
    const r=runtime(),req=ledger();mutate(req);assert.equal(r.execute(req).status,'HALTED');r.store.close();
  }
});
test('document evidence is hashed and transformed without fabricated summary', () => {
  const r=runtime(),req=ledger();req.type='DOCUMENT-INSPECT';
  req.payload={source:{id:'test',location:'test',sha256:hash('abc\ndef')},text:'abc\ndef'};
  const result=r.execute(req);assert.equal(result.output.lines,2); assert.equal(result.output.summaryGenerated,false);
  req.requestId=randomUUID();req.graphId=randomUUID();req.payload.text='changed';
  assert.equal(r.execute(req).errorCode,'SOURCE_HASH_MISMATCH');r.store.close();
});
test('migration divergence blocks; matching output never grants approval', () => {
  const r=runtime(),req=ledger();req.type='MIGRATION-VALIDATE';
  req.payload={source:req.payload.source,modernSource:req.payload.source,legacy:{balance:'1.0000'},modern:{balance:'1.0000'},equivalence:'EXACT-CANONICAL-JSON-V1'};
  const result=r.execute(req);assert.equal(result.status,'COMPLETED');assert.equal(result.output.migrationApproved,false);assert.equal(result.output.sourceOfTruth,'LEGACY');
  req.requestId=randomUUID();req.graphId=randomUUID();req.payload.modern.balance='1.0001';
  assert.equal(r.execute(req).status,'ESCALATED');r.store.close();
});
test('persistent audit resumes prior chain after restart', () => {
  const dir=mkdtempSync(join(tmpdir(),'david-test-')),file=join(dir,'test.sqlite');
  try {
    let r=runtime(new Store(file));r.execute(ledger());const first=r.store.verify();r.store.close();
    r=runtime(new Store(file));assert.deepEqual(r.store.verify(),first);r.execute(ledger());assert.equal(r.store.verify().count,4);r.store.close();
  } finally {rmSync(dir,{recursive:true,force:true});}
});
test('audit and provenance SQL updates are denied and tampering detected', () => {
  const r=runtime();r.execute(ledger());
  assert.throws(()=>r.store.db.exec("UPDATE events SET digest='corrupt'"),/immutable audit/);
  assert.throws(()=>r.store.db.exec("DELETE FROM provenance"),/immutable provenance/);
  r.store.db.exec("DROP TRIGGER immutable_provenance_update; UPDATE provenance SET body='{}'");
  assert.equal(r.execute(ledger()).errorCode,'PROVENANCE_INVALID');r.store.close();
});
test('persistence failure cannot return completion or partially commit evidence', () => {
  const r=runtime();r.store.db.exec("CREATE TRIGGER fail_commit BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'disk failure simulation'); END;");
  assert.equal(r.execute(ledger()).errorCode,'PERSISTENCE_FAILURE');
  assert.equal(r.store.db.prepare('SELECT COUNT(*) AS n FROM provenance').get().n,0);r.store.close();
});
