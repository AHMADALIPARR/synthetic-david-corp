import { randomUUID } from 'node:crypto';
import { amount, canonical, hash, id, object, request, requireThat, source, text } from './contracts.mjs';
import { Store } from './store.mjs';

export const registry = Object.freeze({
  LEDGER: Object.freeze({ tool: 'DOUBLE-ENTRY-VALIDATE', capability: 'ledger.validate', permission: 'ledger:read', dependencies: [] }),
  'MIG-VALID': Object.freeze({ tool: 'LEGACY-MODERN-DIFF', capability: 'migration.validate', permission: 'migration:read', dependencies: [] }),
  DOCUMENT: Object.freeze({ tool: 'DOCUMENT-PARSE', capability: 'document.inspect', permission: 'document:read', dependencies: [] })
});
const routes = Object.freeze({ 'LEDGER-VALIDATE': ['ledger.validate'], 'MIGRATION-VALIDATE': ['migration.validate'], 'DOCUMENT-INSPECT': ['document.inspect'] });
export function selectMinimum(capabilities, agents, permissions) {
  requireThat(Array.isArray(agents) && agents.length <= 64 && agents.every(a => Object.hasOwn(registry, a)), 'UNKNOWN_AGENT');
  requireThat(new Set(agents).size === agents.length, 'SCHEMA_MISMATCH');
  const eligible = [...agents].sort().filter(a => permissions.includes(registry[a].permission));
  const solutions = [];
  // Bound exhaustive selection to the implemented registry, never arbitrary LLM candidates.
  for (let mask = 1; mask < 2 ** eligible.length; mask++) {
    const set = eligible.filter((_, i) => mask & (1 << i));
    if (capabilities.every(c => set.some(a => registry[a].capability === c)) && set.every(a => registry[a].dependencies.every(d => set.includes(d)))) solutions.push(set);
  }
  solutions.sort((a,b) => a.length - b.length || a.join(',').localeCompare(b.join(',')));
  requireThat(solutions.length > 0, 'MISSING_PERMISSION'); return solutions[0];
}
function ledger(payload) {
  object(payload, ['source', 'journalId', 'currency', 'entries']); source(payload.source); id(payload.journalId);
  requireThat(/^[A-Z]{3}$/.test(payload.currency), 'SCHEMA_MISMATCH');
  requireThat(Array.isArray(payload.entries) && payload.entries.length >= 2 && payload.entries.length <= 256, 'SCHEMA_MISMATCH');
  let debits = 0n, credits = 0n; const ids = new Set();
  for (const entry of payload.entries) {
    object(entry, ['entryId', 'accountId', 'debit', 'credit']); id(entry.entryId); id(entry.accountId);
    requireThat(!ids.has(entry.entryId), 'DUPLICATE_ENTRY'); ids.add(entry.entryId);
    const debit = amount(entry.debit), credit = amount(entry.credit);
    requireThat((debit > 0n && credit === 0n) || (credit > 0n && debit === 0n), 'INVALID_ENTRY');
    debits += debit; credits += credit;
  }
  requireThat(debits === credits, 'LEDGER_MISMATCH');
  return { journalId: payload.journalId, currency: payload.currency, debitUnits: String(debits), creditUnits: String(credits), scale: 4, balanced: true, posted: false };
}
function migration(payload) {
  object(payload, ['source', 'modernSource', 'legacy', 'modern', 'equivalence']); source(payload.source); source(payload.modernSource);
  requireThat(payload.equivalence === 'EXACT-CANONICAL-JSON-V1', 'UNKNOWN_EQUIVALENCE');
  requireThat(canonical(payload.legacy) === canonical(payload.modern), 'MIGRATION_DIVERGENCE');
  return { equivalent: true, specification: payload.equivalence, migrationApproved: false, sourceOfTruth: 'LEGACY' };
}
function document(payload) {
  object(payload, ['source', 'text']); source(payload.source); text(payload.text, 7000);
  requireThat(hash(payload.text) === payload.source.sha256, 'SOURCE_HASH_MISMATCH');
  return { characters: payload.text.length, lines: payload.text.split(/\r?\n/).length, summaryGenerated: false };
}
const adapters = Object.freeze({ LEDGER: ledger, 'MIG-VALID': migration, DOCUMENT: document });
export class Runtime {
  constructor({ store = new Store(), principals = {} } = {}) { this.store = store; this.principals = structuredClone(principals); }
  execute(input) {
    const nodes = [], events = []; let req, digest, status = 'HALTED', code, selected = [], output;
    try {
      req = structuredClone(input); request(req); digest = hash(req);
      this.store.verify();
      const cached = this.store.cached(req.requestId);
      if (cached) { requireThat(cached.digest === digest, 'REQUEST_ID_CONFLICT'); return cached.response; }
      requireThat(!this.store.frozen(req.graphId), 'GRAPH_FROZEN');
      requireThat(Object.hasOwn(this.principals, req.requestor), 'UNKNOWN_REQUESTOR');
      const principal = this.principals[req.requestor];
      object(principal, ['agents', 'permissions']);
      requireThat(Array.isArray(principal.permissions) && principal.permissions.every(p => typeof p === 'string'), 'SCHEMA_MISMATCH');
      requireThat(Object.hasOwn(routes, req.type), 'UNKNOWN_TOOL');
      selected = selectMinimum(routes[req.type], principal.agents, principal.permissions);
      for (const agent of selected) {
        const value = adapters[agent](req.payload);
        const timestamp = new Date().toISOString();
        const sources = [req.payload.source, ...(req.payload.modernSource ? [req.payload.modernSource] : [])];
        const node = { id: randomUUID(), result: 'OK', evidence: value, errorCode: 0, errorMessage: '', sources, agentId: agent, toolId: registry[agent].tool, timestamp,
          inputHash: hash(req.payload), outputHash: hash(value), transformation: registry[agent].tool,
          parents: nodes.map(n => n.id), ruleChain: ['PRINCIPAL-ALLOWLIST', 'MINIMUM-CAPABILITY-COVER', 'TYPED-PAYLOAD', 'EXACT-VALIDATION'],
          assumptions: ['Caller-supplied evidence; source authenticity requires an external trusted ingest boundary.'],
          uncertainty: 0, uncertaintyScope: 'deterministic transformation only', riskSignal: 0 };
        nodes.push(node);
        events.push({ eventId: randomUUID(), requestId: req.requestId, traceId: req.traceId, graphId: req.graphId,
          agentId: agent, toolId: node.toolId, inputHash: node.inputHash, outputHash: node.outputHash,
          timestamp, result: 'OK', riskSignal: 0, provenanceId: node.id, provenanceHash: hash(node) });
        output = value;
      }
      status = 'COMPLETED';
    } catch (error) {
      code = error.code || 'INTERNAL_FAILURE';
      status = ['LEDGER_MISMATCH', 'MIGRATION_DIVERGENCE'].includes(code) ? 'ESCALATED' : 'HALTED';
    }
    const response = { status, ...(code ? { errorCode: code } : { output }), agents: selected,
      provenance: nodes, decisionSupportOnly: true, executionAuthority: 'REFERENCE-HARNESS', paymentExecuted: false, migrationApproved: false };
    if (!req || !digest || code === 'REQUEST_ID_CONFLICT' || code === 'AUDIT_CHAIN_INVALID' || code === 'PROVENANCE_INVALID') return response;
    events.push({ eventId: randomUUID(), requestId: req.requestId, traceId: req.traceId, graphId: req.graphId,
      agentId: 'SUPERVISOR', toolId: 'REQUEST-FINALIZE', timestamp: new Date().toISOString(), result: status,
      inputHash: digest, outputHash: hash(response), riskSignal: status === 'COMPLETED' ? 0 : 1, errorCode: code || null });
    try { this.store.commit(req, digest, response, nodes, events, status !== 'COMPLETED'); }
    catch { return { status: 'HALTED', errorCode: 'PERSISTENCE_FAILURE', decisionSupportOnly: true, paymentExecuted: false, migrationApproved: false }; }
    return response;
  }
}
