const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const { join, resolve } = require('node:path');
const { tmpdir } = require('node:os');

const tarball = resolve(process.argv[2] || '');
assert.ok(process.argv[2], 'Usage: npm run test:release -- <release-tarball>');
const directory = fs.mkdtempSync(join(tmpdir(), 'polyglot-release-consumer-'));
try {
  execFileSync(process.execPath, [process.env.npm_execpath, 'install', '--prefix', directory, '--ignore-scripts', '--offline', '--no-audit', '--no-fund', '--cache', join(directory, 'cache'), tarball], { cwd: directory, stdio: 'pipe' });
  const fixture = fs.readFileSync(join(__dirname, '../tests/fixtures/red.png')).toString('base64');
  const test = `
    const assert = require('node:assert/strict');
    const { readFileSync } = require('node:fs');
    const { dirname, join } = require('node:path');
    (async () => {
      const { Polyglot } = require('polyglot-rs');
      const root = dirname(dirname(require.resolve('polyglot-rs')));
      const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
      assert.equal(pkg.scripts, undefined);
      const manifest = JSON.parse(readFileSync(join(root, 'release-manifest.json'), 'utf8'));
      assert.equal(manifest.version, pkg.version);
      assert.equal(manifest.builds.length, 5);
      const data = Buffer.from('${fixture}', 'base64');
      assert.deepEqual((await Polyglot.inspect(data)).formats, ['png']);
      assert.equal((await Polyglot.validate(data, { formats: ['png'] }))[0].valid, true);
      const esm = await import('polyglot-rs');
      assert.equal(esm.Polyglot, Polyglot);
      const compat = await Polyglot.checkCompatibility({ formats: ['png', 'mpeg-ts'] });
      assert.equal(compat.status, 'supported');
      assert.deepEqual(compat.strategies, ['png']);
      const candidate = await Polyglot.create({ inputs: [
        { format: 'opaque-a', data: Buffer.from([0, 255]) },
        { format: 'opaque-b', data: new Uint8Array([42]) },
      ], validate: false });
      assert.deepEqual(candidate.toBuffer(), Buffer.from([0, 255, 42]));
      assert.equal(candidate.report.validated, false);
      // Carrier strategy with a built-in 1x1 carrier from a single payload.
      const carried = await Polyglot.create({ inputs: [{ format: 'payload', data: data }], strategy: 'jpeg' });
      assert.equal(carried.report.strategy, 'jpeg');
      assert.equal(carried.payloadRegion().length, data.length);
      assert.match(carried.byteRange(), /^\\d+@\\d+$/);
    })().catch((error) => { console.error(error); process.exitCode = 1; });
  `;
  execFileSync(process.execPath, ['-e', test], { cwd: directory, stdio: 'inherit' });
  console.log(`Release installed and loaded on ${process.platform}/${process.arch} ${process.version}`);
} finally { fs.rmSync(directory, { recursive: true, force: true }); }
