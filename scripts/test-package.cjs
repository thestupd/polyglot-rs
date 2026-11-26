const { execFileSync } = require('node:child_process');
const { mkdtempSync, rmSync, readFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join, resolve } = require('node:path');
const assert = require('node:assert/strict');

const root = resolve(__dirname, '..');
const temporary = mkdtempSync(join(tmpdir(), 'polyglot-rs-package-'));
// npm_execpath points at npm's JS entrypoint on Windows as well as Unix.
const npm = process.env.npm_execpath;
if (!npm) throw new Error('Run this check with npm run test:package');
require('./package-docs.cjs');
try {
  const packed = JSON.parse(execFileSync(process.execPath, [npm, 'pack', '--workspace=polyglot-rs', '--ignore-scripts', '--json', '--cache', join(temporary, 'npm-cache'), '--pack-destination', temporary], { cwd: root, encoding: 'utf8' }))[0];
  const paths = packed.files.map((file) => file.path);
  for (const path of ['dist/index.js', 'dist/index.d.ts', 'native/index.js', 'docs/feasibility.md', 'README.md', 'LICENSE']) assert.ok(paths.includes(path), `${path} is packaged`);
  assert.ok(paths.some((path) => path.endsWith('.node')), 'native addon is packaged');
  execFileSync(process.execPath, [npm, 'install', '--prefix', temporary, '--ignore-scripts', '--offline', '--no-audit', '--no-fund', '--cache', join(temporary, 'npm-cache'), join(temporary, packed.filename)], { cwd: temporary, stdio: 'pipe' });
  const encoded = readFileSync(join(root, 'tests/fixtures/red.png')).toString('base64');
  const check = `
    const assert = require('node:assert/strict');
    const { Polyglot } = require('polyglot-rs');
    (async () => {
      const data = Buffer.from('${encoded}', 'base64');
      const result = await Polyglot.inspect(data);
      assert.deepEqual(result.formats, ['png']);
      assert.equal(result.metadata.png.validation.valid, true);
      const esm = await import('polyglot-rs');
      assert.equal(esm.Polyglot, Polyglot);
      assert.ok((await esm.Polyglot.validate(data, { formats: ['png'] }))[0].valid);
      assert.equal((await esm.Polyglot.checkCompatibility({ formats: ['png', 'mpeg-ts'] })).status, 'unsupported');
      const candidate = await esm.Polyglot.create({ inputs: [
        { format: 'opaque-a', data: Buffer.from([0, 255]) },
        { format: 'opaque-b', data: new Uint8Array([42]) },
      ], validate: false });
      assert.deepEqual(candidate.toBuffer(), Buffer.from([0, 255, 42]));
      assert.equal(candidate.report.validated, false);
    })().catch((error) => { console.error(error); process.exitCode = 1; });
  `;
  execFileSync(process.execPath, ['-e', check], { cwd: temporary, stdio: 'inherit' });
  console.log('Packed consumer: CommonJS, ESM, inspection, validation, compatibility and candidate creation passed.');
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
