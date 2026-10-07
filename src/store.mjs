import { DatabaseSync } from 'node:sqlite';
import { mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import { hash, requireThat } from './contracts.mjs';
export class Store {
  constructor(filename = ':memory:') {
    if (filename !== ':memory:') mkdirSync(dirname(filename), { recursive: true });
    this.db = new DatabaseSync(filename);
    this.db.exec(`PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;
      CREATE TABLE IF NOT EXISTS events(seq INTEGER PRIMARY KEY, body TEXT NOT NULL, digest TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS provenance(id TEXT PRIMARY KEY, body TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS graphs(id TEXT PRIMARY KEY, frozen INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS requests(id TEXT PRIMARY KEY, digest TEXT NOT NULL, response TEXT NOT NULL);
      CREATE TRIGGER IF NOT EXISTS immutable_events_update BEFORE UPDATE ON events BEGIN SELECT RAISE(ABORT,'immutable audit'); END;
      CREATE TRIGGER IF NOT EXISTS immutable_events_delete BEFORE DELETE ON events BEGIN SELECT RAISE(ABORT,'immutable audit'); END;
      CREATE TRIGGER IF NOT EXISTS immutable_provenance_update BEFORE UPDATE ON provenance BEGIN SELECT RAISE(ABORT,'immutable provenance'); END;
      CREATE TRIGGER IF NOT EXISTS immutable_provenance_delete BEFORE DELETE ON provenance BEGIN SELECT RAISE(ABORT,'immutable provenance'); END;`);
    this.verify();
  }
  frozen(id) { return this.db.prepare('SELECT frozen FROM graphs WHERE id=?').get(id)?.frozen === 1; }
  cached(id) { const row = this.db.prepare('SELECT digest,response FROM requests WHERE id=?').get(id); return row && { digest: row.digest, response: JSON.parse(row.response) }; }
  commit(req, digest, response, nodes, events, freeze) {
    this.db.exec('BEGIN IMMEDIATE');
    try {
      this.verify();
      requireThat(!this.cached(req.requestId), 'REQUEST_REPLAY');
      requireThat(!this.frozen(req.graphId) || response.status === 'HALTED', 'GRAPH_FROZEN');
      let previous = this.db.prepare('SELECT digest FROM events ORDER BY seq DESC LIMIT 1').get()?.digest || '0'.repeat(64);
      for (const node of nodes) this.db.prepare('INSERT INTO provenance VALUES (?,?)').run(node.id, JSON.stringify(node));
      for (const event of events) {
        const body = { ...event, previousHash: previous };
        previous = hash(body);
        this.db.prepare('INSERT INTO events(body,digest) VALUES (?,?)').run(JSON.stringify(body), previous);
      }
      this.db.prepare('INSERT INTO graphs VALUES (?,?) ON CONFLICT(id) DO UPDATE SET frozen=MAX(frozen,excluded.frozen)').run(req.graphId, freeze ? 1 : 0);
      this.db.prepare('INSERT INTO requests VALUES (?,?,?)').run(req.requestId, digest, JSON.stringify(response));
      this.db.exec('COMMIT');
    } catch (error) { this.db.exec('ROLLBACK'); throw error; }
  }
  verify() {
    let previous = '0'.repeat(64), count = 0;
    for (const row of this.db.prepare('SELECT * FROM events ORDER BY seq').all()) {
      const body = JSON.parse(row.body);
      requireThat(body.previousHash === previous && hash(body) === row.digest, 'AUDIT_CHAIN_INVALID');
      if (body.provenanceId) {
        const node = this.db.prepare('SELECT body FROM provenance WHERE id=?').get(body.provenanceId);
        requireThat(node && hash(JSON.parse(node.body)) === body.provenanceHash, 'PROVENANCE_INVALID');
      }
      previous = row.digest; count++;
    }
    return { count, head: previous };
  }
  close() { this.db.close(); }
}
