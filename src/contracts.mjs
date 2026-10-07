import { createHash } from 'node:crypto';
export class ControlError extends Error {
  constructor(code) { super(code); this.code = code; }
}
export function requireThat(ok, code) { if (!ok) throw new ControlError(code); }
export function canonical(value) {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return JSON.stringify(value);
  if (typeof value === 'number') { requireThat(Number.isFinite(value), 'SCHEMA_MISMATCH'); return JSON.stringify(value); }
  if (Array.isArray(value)) return '[' + value.map(canonical).join(',') + ']';
  requireThat(value && Object.getPrototypeOf(value) === Object.prototype, 'SCHEMA_MISMATCH');
  return '{' + Object.keys(value).sort().map(k => JSON.stringify(k) + ':' + canonical(value[k])).join(',') + '}';
}
export const hash = value => createHash('sha256').update(canonical(value)).digest('hex');
export function object(value, required, optional = []) {
  requireThat(value && Object.getPrototypeOf(value) === Object.prototype, 'SCHEMA_MISMATCH');
  requireThat(required.every(k => Object.hasOwn(value, k)) && Object.keys(value).every(k => [...required, ...optional].includes(k)), 'SCHEMA_MISMATCH');
}
export function text(value, max = 128) { requireThat(typeof value === 'string' && value.trim().length > 0 && value.length <= max, 'SCHEMA_MISMATCH'); }
export function id(value) { requireThat(typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value), 'SCHEMA_MISMATCH'); }
export function noSecrets(value) {
  if (Array.isArray(value)) { value.forEach(noSecrets); return; }
  if (value && typeof value === 'object') {
    for (const [k, v] of Object.entries(value)) {
      requireThat(!/password|secret|token|authorization|api.?key|private.?key/i.test(k), 'SECRET_FIELD_FORBIDDEN'); noSecrets(v);
    }
  }
  if (typeof value === 'string') requireThat(!/-----BEGIN .*PRIVATE KEY-----|\bBearer\s+\S+|\bsk-[A-Za-z0-9_-]{16,}/i.test(value), 'SECRET_FIELD_FORBIDDEN');
}
// Decimal(19,4), aligned with PIC S9(15)V9(4). Never use floating point for money.
export function amount(value) {
  requireThat(typeof value === 'string' && /^(0|[1-9][0-9]{0,14})\.[0-9]{4}$/.test(value), 'INVALID_AMOUNT');
  return BigInt(value.replace('.', ''));
}
export function source(value) {
  object(value, ['id', 'location', 'sha256']); text(value.id, 64); text(value.location, 128);
  requireThat(/^[a-f0-9]{64}$/.test(value.sha256), 'PROVENANCE_MISSING');
}
export function request(value) {
  object(value, ['requestId', 'traceId', 'graphId', 'requestor', 'type', 'payload']);
  for (const k of ['requestId', 'traceId', 'graphId']) id(value[k]);
  text(value.requestor, 64); text(value.type, 32); noSecrets(value.payload);
  requireThat(Buffer.byteLength(canonical(value.payload), 'utf8') <= 8192, 'PAYLOAD_TOO_LARGE');
}
