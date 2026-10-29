"""Copy only the reviewed contract source inventory, never keys/build directories."""
import hashlib
import json
import pathlib
import shutil

root = pathlib.Path(__file__).resolve().parents[1]
contract = root.parent / 'atomic-launch-contract-20261029'
vendor = root / 'vendor/evolut-curve'
baseline_path = root / 'evidence/atomic-launch/baseline-vendor-source-manifest.json'
baseline_path.parent.mkdir(parents=True, exist_ok=True)
if not baseline_path.exists():
    shutil.copyfile(root / 'vendor-source-manifest.json', baseline_path)
baseline = json.loads(baseline_path.read_text())
paths = {entry['path'] for entry in baseline['files']}
paths.add('programs/dynamic-bonding-curve/src/instructions/initialize_pool/launch_fee.rs')
files = []
changes = []
old = {entry['path']: entry['sha256'] for entry in baseline['files']}
for relative in sorted(paths):
    source = (contract / relative).resolve()
    destination = (vendor / relative).resolve()
    assert source.is_relative_to(contract.resolve())
    assert destination.is_relative_to(vendor.resolve())
    data = source.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(data)
    files.append({'path': relative, 'sha256': digest})
    if old.get(relative) != digest:
        changes.append({'path': relative, 'baselineSha256': old.get(relative), 'candidateSha256': digest})
manifest = {
    'source': 'atomic-launch-contract-20261029 local candidate; not yet deployed',
    'programId': baseline['programId'],
    'status': 'modified-candidate-source-not-baseline-byte-identical',
    'baselineSource': baseline['source'],
    'baselineManifest': 'evidence/atomic-launch/baseline-vendor-source-manifest.json',
    'sourceFiles': len(files),
    'unchangedFromBaseline': len(files) - len(changes),
    'changes': changes,
    'files': files,
}
(root / 'vendor-source-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps({'files': len(files), 'changedOrAdded': len(changes), 'liveDeploymentChanged': False}))
