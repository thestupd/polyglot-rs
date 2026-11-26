const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const { execFileSync, spawnSync } = require('node:child_process');
const fs = require('node:fs');
const { join, resolve, dirname } = require('node:path');
const { tmpdir } = require('node:os');

const root = resolve(__dirname, '..');
const registry = 'https://registry.npmjs.org/';
const platforms = {
  'darwin-arm64': ['darwin', 'arm64'],
  'darwin-x64': ['darwin', 'x64'],
  'linux-x64-gnu': ['linux', 'x64'],
  'linux-arm64-gnu': ['linux', 'arm64'],
  'win32-x64-msvc': ['win32', 'x64'],
};
const hash = (path, algorithm = 'sha256') => createHash(algorithm).update(fs.readFileSync(path)).digest('hex');
const json = (path) => JSON.parse(fs.readFileSync(path, 'utf8'));
const writeJson = (path, value) => fs.writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
const sourcePackage = () => json(join(root, 'packages/node/package.json'));
const commit = () => execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim();

function npm(args, options = {}) {
  if (!process.env.npm_execpath) throw new Error('Run release commands through npm run');
  return execFileSync(process.execPath, [process.env.npm_execpath, ...args], { cwd: root, encoding: 'utf8', ...options });
}

function record(platform) {
  assert.deepEqual(platforms[platform], [process.platform, process.arch], 'Build target must match the executing platform');
  if (process.platform === 'linux') assert.ok(process.report.getReport().header.glibcVersionRuntime, 'Linux builds require glibc');
  const native = join(root, 'packages/node/native');
  const file = `polyglot-rs.${platform}.node`;
  const metadata = { version: sourcePackage().version, sourceCommit: commit(), platform, file, sha256: hash(join(native, file)), node: process.version };
  writeJson(join(native, 'build.json'), metadata);
  console.log(`Recorded tested build: ${platform} at ${metadata.sourceCommit}`);
}

function findFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isSymbolicLink()) throw new Error(`Release artifacts must not contain symlinks: ${entry.name}`);
    return entry.isDirectory() ? findFiles(path) : [path];
  });
}

function verifyBuilds(artifacts, version, sourceCommit) {
  const files = findFiles(artifacts);
  return Object.keys(platforms).map((platform) => {
    const file = `polyglot-rs.${platform}.node`;
    const matches = files.filter((path) => path.endsWith(`/${file}`) || path.endsWith(`\\${file}`));
    assert.equal(matches.length, 1, `Exactly one binary is required for ${platform}`);
    const path = matches[0];
    const metadata = json(join(dirname(path), 'build.json'));
    assert.equal(metadata.version, version, `${platform}: wrong package version`);
    assert.equal(metadata.sourceCommit, sourceCommit, `${platform}: wrong source commit`);
    assert.equal(metadata.platform, platform, `${platform}: wrong target`);
    assert.equal(metadata.file, file, `${platform}: wrong filename`);
    assert.equal(metadata.sha256, hash(path), `${platform}: binary hash mismatch`);
    return { path, metadata };
  });
}

function prepare(artifacts) {
  const pkg = sourcePackage();
  assert.match(pkg.version, /^\d+\.\d+\.\d+-alpha\.\d+$/, 'This release path publishes alpha versions only');
  const sourceCommit = commit();
  const builds = verifyBuilds(resolve(artifacts), pkg.version, sourceCommit);
  const loaderHash = hash(join(dirname(builds[0].path), 'index.js'));
  for (const build of builds) assert.equal(hash(join(dirname(build.path), 'index.js')), loaderHash, 'Generated native loaders must agree');
  require('./package-docs.cjs');
  const stage = join(root, 'release/package');
  fs.rmSync(stage, { recursive: true, force: true });
  fs.mkdirSync(join(stage, 'native'), { recursive: true });
  for (const file of ['dist', 'src', 'docs', 'README.md', 'LICENSE', 'CHANGELOG.md']) {
    fs.cpSync(join(root, 'packages/node', file), join(stage, file), { recursive: true });
  }
  for (const file of ['index.js', 'index.d.ts']) fs.copyFileSync(join(dirname(builds[0].path), file), join(stage, 'native', file));
  for (const build of builds) fs.copyFileSync(build.path, join(stage, 'native', build.metadata.file));
  // Registry packages contain runtime assets, never workspace lifecycle commands.
  delete pkg.scripts;
  delete pkg.napi;
  writeJson(join(stage, 'package.json'), pkg);
  const manifest = { name: pkg.name, version: pkg.version, sourceCommit, distTag: 'alpha', builds: builds.map((build) => build.metadata) };
  writeJson(join(stage, 'release-manifest.json'), manifest);
  const cache = fs.mkdtempSync(join(tmpdir(), 'polyglot-release-cache-'));
  try {
    const [packed] = JSON.parse(npm(['pack', './release/package', '--ignore-scripts', '--json', '--cache', cache, '--pack-destination', './release']));
    writeJson(join(root, 'release/release.json'), { ...manifest, tarball: packed.filename, integrity: packed.integrity, sha256: hash(join(root, 'release', packed.filename)) });
    console.log(`Prepared ${packed.filename} with ${builds.length} platform binaries`);
  } finally { fs.rmSync(cache, { recursive: true, force: true }); }
}

function check() {
  const manifest = json(join(root, 'release/release.json'));
  const pkg = sourcePackage();
  assert.equal(manifest.name, 'polyglot-rs');
  assert.equal(manifest.version, pkg.version);
  assert.equal(manifest.sourceCommit, commit());
  assert.equal(manifest.distTag, 'alpha');
  assert.equal(manifest.tarball, `polyglot-rs-${pkg.version}.tgz`);
  const path = join(root, 'release', manifest.tarball);
  assert.equal(hash(path), manifest.sha256, 'Prepared tarball was modified');
  const integrity = `sha512-${createHash('sha512').update(fs.readFileSync(path)).digest('base64')}`;
  assert.equal(integrity, manifest.integrity, 'Prepared tarball integrity mismatch');
  assert.deepEqual(manifest.builds.map((build) => build.platform).sort(), Object.keys(platforms).sort());
  return { manifest, path };
}

function publish() {
  const { manifest, path } = check();
  const cache = fs.mkdtempSync(join(tmpdir(), 'polyglot-publish-cache-'));
  try {
    const options = ['--registry', registry, '--cache', cache, '--fetch-retries=0'];
    const identity = npm(['whoami', ...options]).trim();
    console.log(`Publishing identity: ${identity}`);
    const existing = spawnSync(process.execPath, [process.env.npm_execpath, 'view', `polyglot-rs@${manifest.version}`, 'dist.integrity', '--json', ...options], { cwd: root, encoding: 'utf8' });
    if (existing.status === 0) {
      assert.equal(JSON.parse(existing.stdout), manifest.integrity, 'This version already exists with different content');
      console.log(`polyglot-rs@${manifest.version} is already published with matching integrity`);
      return;
    }
    assert.ok(existing.stderr.includes('E404'), `Registry preflight failed: ${existing.stderr}`);
    npm(['publish', path, '--ignore-scripts', '--tag', 'alpha', '--access', 'public', ...options], { stdio: 'inherit' });
    const actual = JSON.parse(npm(['view', `polyglot-rs@${manifest.version}`, 'dist.integrity', '--json', ...options]));
    assert.equal(actual, manifest.integrity, 'Published integrity does not match the tested tarball');
    console.log(`Verified https://www.npmjs.com/package/polyglot-rs/v/${manifest.version}`);
  } finally { fs.rmSync(cache, { recursive: true, force: true }); }
}

if (require.main === module) {
  try {
    const [command, argument] = process.argv.slice(2);
    if (command === 'record') record(argument);
    else if (command === 'prepare') { assert.ok(argument, 'Artifact directory is required'); prepare(argument); }
    else if (command === 'check') { check(); console.log('Release tarball and build manifest verified'); }
    else if (command === 'publish') publish();
    else if (command === 'source-pack') throw new Error('Publish the tested five-platform release tarball. Use npm run release:prepare -- <artifact-directory>; use npm run test:package for a local development preview.');
    else throw new Error('Usage: release.cjs record <platform> | prepare <artifacts> | check | publish');
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}

module.exports = { platforms, verifyBuilds };
