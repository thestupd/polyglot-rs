const test = require('node:test');
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');
const { Polyglot, PolyglotEngine, PolyglotError } = require('../../packages/node');
const fixture = (name) => readFileSync(join(__dirname, '../fixtures', name));

test('Buffer and sliced Uint8Array inspection uses only the view', async () => {
  for (const [name, format] of [['red.png', 'png'], ['black.gif', 'gif'], ['red.ts', 'mpeg-ts']]) {
    const data = fixture(name);
    const original = Buffer.from(data);
    const wrapped = Buffer.concat([Buffer.from('prefix'), data, Buffer.from('suffix')]);
    const view = new Uint8Array(wrapped.buffer, wrapped.byteOffset + 6, data.length);
    for (const input of [data, view]) {
      const result = await Polyglot.inspect(input);
      assert.deepEqual(result.formats, [format]);
      assert.equal(result.sizeBytes, data.length);
      assert.equal(result.metadata[format].source, 'native');
      assert.equal(result.metadata[format].validation.valid, true);
    }
    assert.deepEqual(data, original);
  }
});

test('every requested interpretation is independently validated', async () => {
  const results = await Polyglot.validate(fixture('red.png'), { formats: ['png', 'gif', 'mpeg-ts', 'opaque'] });
  assert.deepEqual(results.map((r) => r.valid), [true, false, false, false]);
  assert.equal(results[3].errors[0], 'No registered native validator for this format');
});

test('malformed signature matches remain inspectable', async () => {
  const result = await Polyglot.inspect(Buffer.from('GIF89a'));
  assert.deepEqual(result.formats, ['gif']);
  assert.equal(result.metadata.gif.validation.valid, false);
  assert.ok(result.metadata.gif.validation.errors.length);
  assert.deepEqual((await Polyglot.inspect(Buffer.from('opaque'))).formats, []);
});

test('invalid arguments and limits yield typed errors', async () => {
  const invalid = (code) => (error) => error instanceof PolyglotError && error.code === code;
  for (const data of [null, 'png', [1, 2, 3], new Uint16Array(2), new Uint8Array(new SharedArrayBuffer(8))]) {
    await assert.rejects(Polyglot.inspect(data), invalid('ERR_INVALID_INPUT'));
  }
  for (const options of [null, { maxInputBytes: 0 }, { maxInputBytes: -1 }, { maxDecodedBytes: 1.5 }, { maxInputBytes: Infinity }]) {
    await assert.rejects(Polyglot.inspect(Buffer.alloc(0), options), invalid('ERR_INVALID_INPUT'));
  }
  await assert.rejects(Polyglot.inspect(fixture('red.png'), { maxInputBytes: 1 }), invalid('ERR_LIMIT_EXCEEDED'));
  for (const options of [undefined, {}, { formats: [] }, { formats: ['PNG'] }, { formats: [null] }]) {
    await assert.rejects(Polyglot.validate(Buffer.alloc(0), options), invalid('ERR_INVALID_INPUT'));
  }
});

test('input is snapshotted before native background processing', async () => {
  const data = fixture('red.png');
  const pending = Polyglot.inspect(data);
  data.fill(0);
  assert.equal((await pending).metadata.png.validation.valid, true);
});

test('custom detection hooks are isolated and never claim native validation', async () => {
  const engine = new PolyglotEngine();
  engine.registerFormat({ name: 'opaque', detect: (buffer) => buffer[0] === 0x89 });
  const result = await engine.inspect(fixture('red.png'));
  assert.deepEqual(result.formats, ['opaque', 'png']);
  assert.equal(result.metadata.opaque.source, 'javascript');
  assert.equal(result.metadata.opaque.validation.valid, false);
  assert.equal((await engine.validate(fixture('red.png'), { formats: ['opaque'] }))[0].valid, false);
  assert.deepEqual((await Polyglot.inspect(fixture('red.png'))).formats, ['png']);
  assert.throws(() => engine.registerFormat({ name: 'png', detect: () => true }), { code: 'ERR_DUPLICATE_FORMAT' });
  assert.throws(() => engine.registerFormat({ name: 'opaque', detect: () => true }), { code: 'ERR_DUPLICATE_FORMAT' });
  assert.equal(engine.unregisterFormat('opaque'), true);
});

test('all matching hooks are reported and mutating hooks cannot affect each other', async () => {
  const engine = new PolyglotEngine();
  engine.registerFormat({ name: 'mutating', detect: (b) => { b.fill(0); return true; } });
  engine.registerFormat({ name: 'still-png', detect: async (b) => b[0] === 0x89 });
  const data = fixture('red.png');
  const result = await engine.inspect(data);
  assert.deepEqual(result.formats, ['mutating', 'png', 'still-png']);
  assert.equal(data[0], 0x89);
  const broken = new PolyglotEngine();
  broken.registerFormat({ name: 'broken', detect: () => { throw new Error('hook failure'); } });
  await assert.rejects(broken.inspect(data), { code: 'ERR_CUSTOM_DETECTION' });
  broken.unregisterFormat('broken');
  broken.registerFormat({ name: 'nonboolean', detect: () => 'yes' });
  await assert.rejects(broken.inspect(data), { code: 'ERR_CUSTOM_DETECTION' });
});

test('native parsing leaves the event loop available', async () => {
  const engine = new PolyglotEngine();
  // Keep a worker busy enough that a main-thread timer must run first.
  const data = Buffer.alloc(188 * 100_000, 0xff);
  for (let i = 0; i < data.length; i += 188) data.set([0x47, 0x1f, 0xff, 0x10], i);
  const timer = new Promise((resolve) => setTimeout(() => resolve('timer'), 0));
  const pending = engine.validate(data, { formats: ['mpeg-ts'] });
  assert.equal(await Promise.race([timer, pending.then(() => 'native')]), 'timer');
  assert.equal((await pending)[0].valid, true);
});

test('ESM named imports are available', async () => {
  const esm = await import('../../packages/node/dist/index.js');
  assert.equal(esm.Polyglot, Polyglot);
});
