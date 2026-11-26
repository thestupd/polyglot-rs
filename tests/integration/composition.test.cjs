const test = require('node:test');
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');
const { Polyglot, PolyglotEngine, PolyglotResult } = require('../../packages/node');
const fixture = (name) => readFileSync(join(__dirname, '../fixtures', name));
const opaque = () => [
  { format: 'opaque-a', data: Buffer.from([0, 255]), label: 'header' },
  { format: 'opaque-b', data: new Uint8Array([42]) },
];

test('compatibility reports constraints and never promotes concatenation to support', async () => {
  for (const formats of [['png', 'mpeg-ts'], ['gif', 'mpeg-ts'], ['mpeg-ts', 'png']]) {
    const result = await Polyglot.checkCompatibility({ formats });
    assert.equal(result.status, 'unsupported');
    assert.deepEqual(result.strategies, []);
    assert.ok(result.limitations.length);
  }
  const unknown = await Polyglot.checkCompatibility({ formats: ['jpeg', 'zip'] });
  assert.equal(unknown.status, 'conditional');
  assert.deepEqual(unknown.strategies, ['append']);
  assert.ok(unknown.limitations.some((l) => l.includes('opaque')));
  for (const strategy of ['embed', 'overlap']) {
    const result = await Polyglot.checkCompatibility({ formats: ['png', 'mpeg-ts'], strategy });
    assert.equal(result.status, 'unsupported');
  }
  const requirements = Polyglot.listStrategies();
  assert.deepEqual(Object.keys(requirements), ['append', 'embed', 'overlap']);
  assert.equal(requirements.append.minInputs, 2);
  assert.equal(requirements.embed.requiresNativeHandlers, true);
});

test('opaque binary assembly is explicit, deterministic and unvalidated', async () => {
  await assert.rejects(Polyglot.create({ inputs: opaque() }), { code: 'ERR_INVALID_COMPOSITION' });
  const inputs = opaque();
  const pending = Polyglot.create({ inputs, validate: false });
  inputs[0].data.fill(0);
  const result = await pending;
  assert.ok(result instanceof PolyglotResult);
  assert.deepEqual(result.toBuffer(), Buffer.from([0, 255, 42]));
  assert.equal(result.sizeBytes, 3);
  assert.equal(result.report.sizeBytes, 3);
  assert.equal(result.report.validated, false);
  assert.deepEqual(result.report.validation, []);
  assert.equal(result.report.inputs[0].label, 'header');
  assert.equal(result.report.inputs[0].validation, null);
  assert.equal(result.report.compatibility.status, 'conditional');
  result.toBuffer().fill(0);
  assert.deepEqual(result.toBuffer(), (await Polyglot.create({ inputs: opaque(), strategy: 'append', validate: false })).toBuffer());
});

test('native output validation runs over the complete buffer', async () => {
  const data = fixture('red.ts');
  const result = await Polyglot.create({ inputs: [
    { format: 'mpeg-ts', data: data.subarray(0, 188 * 4) },
    { format: 'mpeg-ts', data: data.subarray(188 * 4) },
  ] });
  assert.deepEqual(result.toBuffer(), data);
  assert.equal(result.report.validated, true);
  assert.equal(result.report.validation.length, 1);
  assert.equal(result.report.validation[0].valid, true);
  assert.ok(result.report.inputs.every((i) => i.validation.valid));
  assert.equal(result.report.compatibility.status, 'conditional');
  assert.ok(result.report.warnings.some((w) => w.includes('multiple distinct')));
  // Both inputs are valid in isolation, but their repeated counters break the
  // complete transport stream. Appending cannot bypass output validation.
  await assert.rejects(Polyglot.create({ inputs: [
    { format: 'mpeg-ts', data }, { format: 'mpeg-ts', data },
  ] }), { code: 'ERR_INVALID_COMPOSITION' });
});

test('unsupported combinations and placeholder strategies cannot be forced', async () => {
  const inputs = [{ format: 'png', data: fixture('red.png') }, { format: 'mpeg-ts', data: fixture('red.ts') }];
  for (const validate of [true, false]) {
    await assert.rejects(Polyglot.create({ inputs, validate }), { code: 'ERR_UNSUPPORTED_COMBINATION' });
  }
  for (const strategy of ['embed', 'overlap']) {
    await assert.rejects(Polyglot.create({ inputs: opaque(), strategy, validate: false }), { code: 'ERR_UNSUPPORTED_COMBINATION' });
  }
  await assert.rejects(Polyglot.create({ inputs: opaque(), strategy: 'absent', validate: false }), { code: 'ERR_UNKNOWN_STRATEGY' });
});

test('aggregate input and output limits are enforced separately', async () => {
  await assert.rejects(Polyglot.create({ inputs: opaque(), maxInputBytes: 2, validate: false }), { code: 'ERR_LIMIT_EXCEEDED' });
  await assert.rejects(Polyglot.create({ inputs: opaque(), maxOutputBytes: 2, validate: false }), { code: 'ERR_LIMIT_EXCEEDED' });
  const result = await Polyglot.create({ inputs: opaque(), maxInputBytes: 3, maxOutputBytes: 3, validate: false });
  assert.equal(result.sizeBytes, 3);
});

test('creation validates runtime options', async () => {
  for (const options of [null, {}, { inputs: [] }, { inputs: [opaque()[0]] }, { inputs: [null, null] },
    { inputs: opaque(), validate: 'false' }, { inputs: opaque(), maxOutputBytes: 0 },
    { inputs: opaque(), strategy: 2 }, { inputs: [{ format: 'PNG', data: Buffer.alloc(1) }, opaque()[0]] },
    { inputs: [{ format: 'a', data: 'text' }, opaque()[0]] },
    { inputs: [{ format: 'a', data: Buffer.alloc(1), label: 123 }, opaque()[0]] },
  ]) {
    await assert.rejects(Polyglot.create(options), { code: 'ERR_INVALID_INPUT' });
  }
  for (const options of [null, {}, { formats: [] }, { formats: ['png'] }, { formats: ['PNG', 'gif'] }]) {
    await assert.rejects(Polyglot.checkCompatibility(options), { code: 'ERR_INVALID_INPUT' });
  }
  await assert.rejects(Polyglot.checkCompatibility({ formats: ['png', 'gif'], strategy: 'absent' }), { code: 'ERR_UNKNOWN_STRATEGY' });
});

test('JavaScript hooks cannot enable native composition or validation', async () => {
  const engine = new PolyglotEngine();
  let called = false;
  engine.registerFormat({ name: 'opaque-a', detect: () => { called = true; return true; } });
  await assert.rejects(engine.create({ inputs: opaque() }), { code: 'ERR_INVALID_COMPOSITION' });
  assert.equal(called, false);
  assert.equal((await engine.create({ inputs: opaque(), validate: false })).report.validated, false);
});
