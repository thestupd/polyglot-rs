const test = require('node:test');
const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const fs = require('node:fs');
const { join } = require('node:path');
const { tmpdir } = require('node:os');
const { platforms, verifyBuilds } = require('../../scripts/release.cjs');

function artifacts(t) {
  const root = fs.mkdtempSync(join(tmpdir(), 'polyglot-artifact-test-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  // Synthetic artifact metadata tests, not production native binaries.
  for (const platform of Object.keys(platforms)) {
    const path = join(root, platform);
    fs.mkdirSync(path);
    const file = `polyglot-rs.${platform}.node`;
    fs.writeFileSync(join(path, file), platform);
    const sha256 = createHash('sha256').update(platform).digest('hex');
    fs.writeFileSync(join(path, 'build.json'), JSON.stringify({ version: '0.1.0-alpha.1', sourceCommit: 'commit-a', platform, file, sha256 }));
  }
  return root;
}

test('release assembly refuses an incomplete platform set', (t) => {
  const root = artifacts(t);
  fs.rmSync(join(root, 'win32-x64-msvc'), { recursive: true });
  assert.throws(() => verifyBuilds(root, '0.1.0-alpha.1', 'commit-a'), /Exactly one binary.*win32/);
});

test('release assembly refuses binaries changed after their build record', (t) => {
  const root = artifacts(t);
  fs.appendFileSync(join(root, 'darwin-arm64/polyglot-rs.darwin-arm64.node'), 'changed');
  assert.throws(() => verifyBuilds(root, '0.1.0-alpha.1', 'commit-a'), /binary hash mismatch/);
});

test('release assembly refuses mixed source commits and versions', (t) => {
  const root = artifacts(t);
  assert.equal(verifyBuilds(root, '0.1.0-alpha.1', 'commit-a').length, 5);
  assert.throws(() => verifyBuilds(root, '0.1.0-alpha.1', 'commit-b'), /wrong source commit/);
  assert.throws(() => verifyBuilds(root, '0.1.0-alpha.2', 'commit-a'), /wrong package version/);
});

test('release assembly refuses duplicate platform artifacts', (t) => {
  const root = artifacts(t);
  fs.cpSync(join(root, 'darwin-arm64'), join(root, 'duplicate'), { recursive: true });
  assert.throws(() => verifyBuilds(root, '0.1.0-alpha.1', 'commit-a'), /Exactly one binary.*darwin-arm64/);
});
