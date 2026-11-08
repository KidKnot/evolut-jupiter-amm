import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
const root=new URL('../',import.meta.url);
const read=async p=>JSON.parse(await readFile(new URL(p,root),'utf8'));
const baseline=await read('evidence/irys-sol-20261108/baseline-abi.json');
const current=await read('vendor/evolut-curve/idl/evolut_curve.mainnet.json');
assert.equal(current.address,baseline.programId);
const counts={};
for(const kind of ['instructions','accounts','types']) {
  const byName=new Map(current[kind].map(value=>[value.name,value]));
  for(const expected of baseline[kind]) {
    assert.ok(byName.has(expected.name),`Missing ${kind}: ${expected.name}`);
    const hash=createHash('sha256').update(JSON.stringify(byName.get(expected.name))).digest('hex');
    assert.equal(hash,expected.sha256,`Changed ${kind}: ${expected.name}`);
  }
  counts[kind]=baseline[kind].length;
}
assert.equal(current.instructions.length,baseline.instructions.length+4);
assert.equal(current.accounts.length,baseline.accounts.length);
assert.equal(current.types.length,baseline.types.length+1);
console.log(JSON.stringify({verified:true,preserved:counts,additionalInstructions:4,additionalEventTypes:1,deployed:false,networkCalls:0}));
