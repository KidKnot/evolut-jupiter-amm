"""Query public advisory metadata; send package names/versions only, never source."""
import datetime
import hashlib
import json
import pathlib
import time
import tomllib
import urllib.request

root = pathlib.Path(__file__).resolve().parent
workspace = root.parents[1]
lockfiles = {
    'deployed-contract': workspace / 'output/matched-fee-contract-20261028/Cargo.lock',
    'jupiter-adapter-and-tests': workspace / 'evolut-jupiter-amm/Cargo.lock',
}
packages = {}
locks = {}
for scope, path in lockfiles.items():
    raw = path.read_bytes()
    locks[scope] = hashlib.sha256(raw).hexdigest()
    data = tomllib.loads(raw.decode('utf-8'))
    for p in data['package']:
        if not p.get('source', '').startswith('registry+'):
            continue
        packages.setdefault((p['name'], p['version']), []).append(scope)

rows = list(packages.items())
findings = []
for offset in range(0, len(rows), 100):
    chunk = rows[offset:offset + 100]
    payload = {'queries': [{'package': {'ecosystem': 'crates.io', 'name': k[0]}, 'version': k[1]} for k, _ in chunk]}
    request = urllib.request.Request('https://api.osv.dev/v1/querybatch', data=json.dumps(payload).encode(), headers={'Content-Type': 'application/json'}, method='POST')
    with urllib.request.urlopen(request, timeout=40) as response:
        result = json.load(response)
    assert len(result['results']) == len(chunk)
    for ((name, version), scopes), item in zip(chunk, result['results']):
        if item.get('vulns'):
            findings.append({'name': name, 'version': version, 'scopes': scopes, 'advisories': item['vulns']})
    print(f'Checked {min(offset + 100, len(rows))}/{len(rows)} public crate versions', flush=True)

advisory_ids = sorted({v['id'] for f in findings for v in f['advisories']})
advisories = {}
for aid in advisory_ids:
    with urllib.request.urlopen('https://api.osv.dev/v1/vulns/' + aid, timeout=30) as response:
        advisories[aid] = json.load(response)
output = {
    'checkedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'source': 'OSV public advisory API; transitive lockfile inventory, not reachability proof',
    'lockfileSha256': locks, 'uniqueRegistryPackageVersions': len(rows),
    'findings': findings, 'advisories': advisories,
    'limitations': ['Includes build/test and target-specific crates; actual feature reachability needs separate review.', 'Git/path dependencies and npm packages not covered.', 'No source code, credentials or private endpoints transmitted.'],
}
(root / 'dependency-advisories.json').write_text(json.dumps(output, indent=2) + '\n', encoding='utf-8')
for finding in findings:
    print(json.dumps({**finding, 'summaries': [advisories[v['id']].get('summary') for v in finding['advisories']]}))
print(f'{len(findings)} package-version matches; {len(advisories)} distinct advisories')
