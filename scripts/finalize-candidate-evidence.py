"""Bind the two completed offline runs to this candidate; no network/deploy action."""
import datetime
import hashlib
import json
import pathlib
import re

root = pathlib.Path(__file__).resolve().parents[1]
sha = lambda data: hashlib.sha256(data).hexdigest()
runs = []
for name, binary, log in [
    ('patched-adapter-captured-baseline-sbf', 'evidence/atomic-launch/captured-baseline-885d9c88.so', 'evidence/atomic-launch/patched-dependencies-baseline-sbf-tests.log'),
    ('patched-adapter-local-candidate-sbf', 'tests/fixtures/evolut.so', 'evidence/atomic-launch/candidate-sbf-tests.log'),
]:
    output = (root / log).read_text(encoding='utf-8-sig')
    counts = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed;', output)
    passed = sum(int(item[0]) for item in counts)
    failed = sum(int(item[1]) for item in counts)
    assert passed == 27 and failed == 0, (name, counts)
    data = (root / binary).read_bytes()
    runs.append({'name': name, 'binary': binary, 'binaryBytes': len(data), 'binarySha256': sha(data), 'log': log, 'logSha256': sha((root / log).read_bytes()), 'passed': passed, 'failed': failed})
evidence = {
    'preparedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'status': 'local-atomic-launch-candidate-tested-not-deployed-or-published',
    'independentAudit': 'none; internal testing only',
    'sourcePermission': 'unresolved; unchanged by local remediation',
    'rust': 'rustc 1.94.1 stable',
    'platform': 'Windows x64 MSVC',
    'testCommand': 'node run-cargo.mjs test --locked --offline --all-targets --jobs 2',
    'runs': runs,
    'formatCommand': 'node run-cargo.mjs fmt --package evolut-jupiter-amm -- --check',
    'formatting': 'passed; vendored source not reformatted',
    'accountSnapshotSha256': sha((root / 'tests/fixtures/captured/mainnet.json').read_bytes()),
    'accountSnapshotChanged': False,
    'adapterLockfileSha256': sha((root / 'Cargo.lock').read_bytes()),
    'vendoredContractLockfileSha256': sha((root / 'vendor/evolut-curve/Cargo.lock').read_bytes()),
    'dependencyReview': 'docs/DEPENDENCY-REMEDIATION.md',
    'historicalValidation': 'evidence/atomic-launch/baseline-validation.json',
    'limitations': [
        'No live upgrade or mainnet transactions',
        'Candidate bytecode is locally built, not a fresh mainnet capture',
        '1% and two-segment execution states are synthetic',
        'Final Jupiter CPI/multi-hop integration unverified',
        'Source-sharing permission and external audit acceptance remain unresolved',
        'Maintenance and non-applicable/feature-dependent advisory matches remain recorded',
    ],
    'transactionsSubmitted': 0,
}
(root / 'evidence/validation.json').write_text(json.dumps(evidence, indent=2) + '\n')
ignored = {'.git', 'target', 'node_modules', 'build-logs', '.cache', '__pycache__'}
entries = []
for path in sorted(root.rglob('*')):
    rel = path.relative_to(root)
    if any(part in ignored for part in rel.parts) or not path.is_file() or rel.as_posix() == 'package-manifest.json':
        continue
    assert not path.is_symlink(), path
    data = path.read_bytes()
    entries.append({'path': rel.as_posix(), 'bytes': len(data), 'sha256': sha(data)})
manifest = {
    'schemaVersion': 1,
    'preparedDate': '2026-10-29',
    'status': 'atomic-launch-candidate-tested-not-deployed-source-rights-unresolved',
    'programId': 'EVoLutQjaoK3fkQh1ULbQzqVruU4yYEdSsqZB5djTTQZ',
    'programUnderTest': 'tests/fixtures/program-under-test.json',
    'historicalBaseline': 'evidence/atomic-launch/baseline-validation.json',
    'files': entries,
}
(root / 'package-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps({'runs': [{'name': run['name'], 'passed': run['passed']} for run in runs], 'packagedFiles': len(entries)}))
