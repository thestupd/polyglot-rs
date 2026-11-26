const test = require('node:test');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const { join } = require('node:path');
const { readFileSync } = require('node:fs');
const { Polyglot } = require('../../packages/node');
const ffmpeg = process.env.FFMPEG_PATH || 'ffmpeg';
const probe = spawnSync(ffmpeg, ['-version']);
const available = !probe.error && probe.status === 0;
if (!available && process.env.REQUIRE_EXTERNAL_PARSERS === '1') throw new Error('FFmpeg is required for this verification run');

for (const [name, format, expected] of [
  ['red.png', 'png', Buffer.from([255, 0, 0, 255, 0, 0, 255, 0, 0, 255, 0, 0])],
  ['black.gif', 'gif', Buffer.from([0, 0, 0])],
]) {
  test(`independent FFmpeg decoder verifies ${format} pixels`, { skip: !available && 'FFmpeg not installed' }, async () => {
    const path = join(__dirname, '../fixtures', name);
    const [result] = await Polyglot.validate(readFileSync(path), { formats: [format] });
    assert.equal(result.valid, true);
    const decode = spawnSync(ffmpeg, ['-hide_banner', '-loglevel', 'error', '-xerror', '-err_detect', 'explode', '-i', path, '-frames:v', '1', '-f', 'rawvideo', '-pix_fmt', 'rgb24', 'pipe:1']);
    assert.equal(decode.status, 0, decode.stderr?.toString());
    assert.deepEqual(decode.stdout, expected);
  });
}

test('independent MPEG-TS demuxing and MPEG-2 video decoding', { skip: !available && 'FFmpeg not installed' }, async () => {
  const path = join(__dirname, '../fixtures/red.ts');
  const source = readFileSync(path);
  const [result] = await Polyglot.validate(source, { formats: ['mpeg-ts'] });
  assert.equal(result.valid, true, result.errors.join('\n'));
  const candidate = await Polyglot.create({ inputs: [
    { format: 'mpeg-ts', data: source.subarray(0, 188 * 4) },
    { format: 'mpeg-ts', data: source.subarray(188 * 4) },
  ] });
  assert.deepEqual(candidate.toBuffer(), source);
  const decode = spawnSync(ffmpeg, ['-hide_banner', '-loglevel', 'error', '-xerror', '-err_detect', 'explode', '-i', 'pipe:0', '-f', 'rawvideo', '-pix_fmt', 'rgb24', 'pipe:1'], { input: candidate.toBuffer() });
  assert.equal(decode.status, 0, decode.stderr?.toString());
  assert.equal(decode.stdout.length, 16 * 16 * 3 * 10);
  for (let i = 0; i < decode.stdout.length; i += 3) {
    assert.ok(decode.stdout[i] >= 245, 'red channel');
    assert.ok(decode.stdout[i + 1] <= 10, 'green channel');
    assert.ok(decode.stdout[i + 2] <= 10, 'blue channel');
  }
});
