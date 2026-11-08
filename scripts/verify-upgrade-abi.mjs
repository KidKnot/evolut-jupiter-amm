import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
const root = new URL('../', import.meta.url);
const read = async p => JSON.parse(await readFile(new URL(p, root), 'utf8'));
const hash = value => createHash('sha256').update(JSON.stringify(value)).digest('hex');
const normalize = value => Array.isArray(value) ? value.map(normalize) : value && typeof value === 'object'
  ? Object.fromEntries(Object.entries(value).filter(([key]) => key !== 'docs').map(([key, item]) => [key, normalize(item)])) : value;
const deployed = await read('evidence/irys-sol-20261108/baseline-abi.json');
const baseline = await read('evidence/buybacks-20261108/baseline-abi.json');
const current = await read('vendor/evolut-curve/idl/evolut_curve.mainnet.json');
assert.equal(current.address, baseline.programId); assert.equal(current.address, deployed.programId);
for (const kind of ['instructions', 'accounts', 'types']) {
  const byName = new Map(baseline[kind].map(x => [x.name, x]));
  for (const old of deployed[kind]) assert.equal(byName.get(old.name)?.originalSha256, old.sha256, `Changed deployed baseline: ${old.name}`);
}
const preserved = {};
for (const kind of ['instructions', 'accounts', 'types', 'events']) {
  const byName = new Map(current[kind].map(x => [x.name, x]));
  for (const old of baseline[kind]) {
    assert.ok(byName.has(old.name), `Missing ${kind}: ${old.name}`);
    assert.equal(hash(normalize(byName.get(old.name))), old.semanticSha256, `Changed wire ABI: ${old.name}`);
  }
  preserved[kind] = baseline[kind].length;
}
assert.equal(current.instructions.length, 40); assert.equal(current.accounts.length, 9);
assert.equal(current.types.length, 55); assert.equal(current.events.length, 23);
console.log(JSON.stringify({ verified: true, preserved, normalization: 'docs only',
  graduationRemainingAccountsChanged: true, deployed: false, networkCalls: 0 }));
