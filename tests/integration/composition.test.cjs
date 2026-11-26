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

test('compatibility reports a carrier strategy and never promotes append', async () => {
  for (const [formats, strategy] of [
    [['png', 'mpeg-ts'], 'png'],
    [['gif', 'mpeg-ts'], 'gif'],
    [['jpeg', 'mpeg-ts'], 'jpeg'],
    [['png', 'segment'], 'png'],
    [['jpeg', 'zip'], 'jpeg'],
  ]) {
    const result = await Polyglot.checkCompatibility({ formats });
    assert.equal(result.status, 'supported');
    assert.deepEqual(result.strategies, [strategy]);
    assert.ok(result.limitations.some((l) => l.includes('length@offset')));
    const append = await Polyglot.checkCompatibility({ formats, strategy: 'append' });
    assert.equal(append.status, 'unsupported');
    assert.deepEqual(append.strategies, []);
  }
  // The carrier image must be the first input.
  const swapped = await Polyglot.checkCompatibility({ formats: ['mpeg-ts', 'png'] });
  assert.equal(swapped.status, 'unsupported');
  assert.deepEqual(swapped.strategies, []);
  // Two non-carrier formats fall back to a conditional append candidate.
  const opaquePair = await Polyglot.checkCompatibility({ formats: ['zip', 'rar'] });
  assert.equal(opaquePair.status, 'conditional');
  assert.deepEqual(opaquePair.strategies, ['append']);
  for (const strategy of ['embed', 'overlap']) {
    const result = await Polyglot.checkCompatibility({ formats: ['png', 'mpeg-ts'], strategy });
    assert.equal(result.status, 'unsupported');
  }
  const requirements = Polyglot.listStrategies();
  assert.deepEqual(Object.keys(requirements), ['append', 'png', 'gif', 'jpeg', 'embed', 'overlap']);
  assert.equal(requirements.png.minInputs, 2);
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
  assert.deepEqual(result.report.regions, []);
  assert.equal(result.byteRange(), undefined);
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

test('png carrier embeds the payload unchanged and the whole file stays a valid PNG', async () => {
  const [png, ts] = [fixture('red.png'), fixture('red.ts')];
  // The payload format name is arbitrary and the payload is never validated.
  const result = await Polyglot.create({ inputs: [{ format: 'png', data: png }, { format: 'segment', data: ts }] });
  const data = result.toBuffer();
  assert.equal(result.report.strategy, 'png');
  assert.equal(result.report.compatibility.status, 'supported');
  assert.equal(result.report.validated, true);
  assert.deepEqual(result.report.validation.map((v) => [v.format, v.valid]), [['png', true]]);
  assert.equal(result.report.inputs[0].validation.valid, true);
  assert.equal(result.report.inputs[1].validation, null);
  assert.ok(result.report.warnings.some((w) => w.toLowerCase().includes('carried')));
  assert.deepEqual(data.subarray(0, 33), png.subarray(0, 33));
  assert.equal(data.subarray(37, 41).toString('latin1'), 'paTs');
  assert.deepEqual(result.region('segment'), { format: 'segment', offset: 41, length: ts.length });
  assert.deepEqual(result.region('png'), { format: 'png', offset: 0, length: data.length });
  assert.deepEqual(result.payloadRegion(), { format: 'segment', offset: 41, length: ts.length });
  assert.equal(result.byteRange(), `${ts.length}@41`);
  assert.deepEqual(data.subarray(41, 41 + ts.length), ts);
  assert.equal(data.length, png.length + ts.length + 12);
  const [whole] = await Polyglot.validate(data, { formats: ['png'] });
  assert.equal(whole.valid, true, whole.errors.join('; '));
});

test('gif and jpeg carriers append the payload after their terminator', async () => {
  const ts = fixture('red.ts');
  for (const [carrier, name] of [['gif', 'black.gif'], ['jpeg', 'red.jpg']]) {
    const image = fixture(name);
    // Even a payload named 'mpeg-ts' is carried unchanged, not validated.
    const result = await Polyglot.create({ inputs: [{ format: carrier, data: image }, { format: 'mpeg-ts', data: ts }] });
    assert.equal(result.report.strategy, carrier);
    assert.deepEqual(result.toBuffer(), Buffer.concat([image, ts]));
    assert.deepEqual(result.report.regions, [
      { format: carrier, offset: 0, length: image.length },
      { format: 'mpeg-ts', offset: image.length, length: ts.length },
    ]);
    assert.deepEqual(result.report.validation.map((v) => v.format), [carrier]);
    assert.equal(result.report.inputs[1].validation, null);
    assert.equal(result.byteRange(), `${ts.length}@${image.length}`);
  }
});

test('explicit carrier selection, and region() returns a copy', async () => {
  const [gif, ts] = [fixture('black.gif'), fixture('red.ts')];
  const auto = await Polyglot.create({ inputs: [{ format: 'gif', data: gif }, { format: 'segment', data: ts }] });
  const explicit = await Polyglot.create({ inputs: [{ format: 'gif', data: gif }, { format: 'segment', data: ts }], strategy: 'gif' });
  assert.deepEqual(explicit.toBuffer(), auto.toBuffer());
  const region = auto.region('segment');
  region.offset = 0;
  assert.equal(auto.region('segment').offset, gif.length, 'region() returns a copy');
});

test('only the carrier is validated; any payload bytes are carried', async () => {
  const png = fixture('red.png');
  // Truncated, non-media or empty payloads are carried without error.
  for (const payload of [fixture('red.ts').subarray(1), Buffer.from('arbitrary bytes'), Buffer.alloc(0)]) {
    const result = await Polyglot.create({ inputs: [{ format: 'png', data: png }, { format: 'segment', data: payload }] });
    assert.equal(result.report.validated, true);
    assert.equal(result.payloadRegion().length, payload.length);
  }
  // A corrupt carrier is rejected; the payload is still never checked.
  const badPng = Buffer.from(png);
  badPng[20] ^= 0xff;
  await assert.rejects(Polyglot.create({ inputs: [{ format: 'png', data: badPng }, { format: 'segment', data: fixture('red.ts') }] }), { code: 'ERR_INVALID_COMPOSITION' });
  // The output budget covers carrier + payload.
  await assert.rejects(Polyglot.create({ inputs: [{ format: 'gif', data: fixture('black.gif') }, { format: 'segment', data: fixture('red.ts') }], maxOutputBytes: 10 }), { code: 'ERR_LIMIT_EXCEEDED' });
});

test('a single payload gets a built-in 1x1 carrier', async () => {
  const ts = fixture('red.ts');
  for (const carrier of ['png', 'gif', 'jpeg']) {
    const result = await Polyglot.create({ inputs: [{ format: 'segment', data: ts }], strategy: carrier });
    assert.equal(result.report.strategy, carrier);
    assert.equal(result.report.compatibility.status, 'supported');
    const [image] = await Polyglot.validate(result.toBuffer().subarray(0, result.region(carrier).length), { formats: [carrier] });
    assert.equal(image.valid, true, image.errors.join('; '));
    const payload = result.payloadRegion();
    assert.equal(payload.format, 'segment');
    assert.equal(payload.length, ts.length);
    assert.deepEqual(result.toBuffer().subarray(payload.offset, payload.offset + payload.length), ts);
  }
  // 'auto' with a single input defaults to a png carrier.
  const auto = await Polyglot.create({ inputs: [{ format: 'segment', data: ts }] });
  assert.equal(auto.report.strategy, 'png');
  // A single input with a non-carrier strategy is still an error.
  await assert.rejects(Polyglot.create({ inputs: [{ format: 'segment', data: ts }], strategy: 'append' }), { code: 'ERR_INVALID_INPUT' });
});

test('unsupported combinations and placeholder strategies cannot be forced', async () => {
  const inputs = [{ format: 'png', data: fixture('red.png') }, { format: 'mpeg-ts', data: fixture('red.ts') }];
  for (const validate of [true, false]) {
    await assert.rejects(Polyglot.create({ inputs, validate, strategy: 'append' }), { code: 'ERR_UNSUPPORTED_COMBINATION' });
    await assert.rejects(Polyglot.create({ inputs, validate, strategy: 'gif' }), { code: 'ERR_UNSUPPORTED_COMBINATION' });
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
  for (const options of [null, {}, { inputs: [] }, { inputs: [null, null] },
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
