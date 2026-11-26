"""Research controls only; not a supported composition API.

Run after npm run build: python3 scripts/feasibility.py
Requires FFmpeg. JSON output records complete decoding, not just probing.
"""
from pathlib import Path
import json
import shutil
import struct
import subprocess
import tempfile
import zlib

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / 'tests' / 'fixtures'


def chunk(kind, data):
    return (struct.pack('>I', len(data)) + kind + data
            + struct.pack('>I', zlib.crc32(kind + data)))


def decode(path, fmt, image):
    args = ['ffmpeg', '-hide_banner', '-loglevel', 'error', '-xerror', '-err_detect', 'explode']
    if fmt:
        args += ['-f', fmt]
    args += ['-i', str(path), '-fps_mode', 'passthrough', '-f', 'rawvideo', '-pix_fmt', 'rgb24', 'pipe:1']
    result = subprocess.run(args, capture_output=True, timeout=30)
    pixels = result.stdout
    video_matches = len(pixels) == 16 * 16 * 3 * 10 and all(
        pixels[i] >= 245 and pixels[i + 1] <= 10 and pixels[i + 2] <= 10
        for i in range(0, len(pixels), 3)
    )
    return {
        'exit': result.returncode, 'decodedBytes': len(pixels),
        'imageMatches': result.returncode == 0 and pixels == image,
        'videoMatches': result.returncode == 0 and video_matches,
        'hasErrors': bool(result.stderr.strip()),
    }


def native(path, formats):
    script = """
      const { Polyglot } = require('./packages/node');
      const { readFileSync } = require('node:fs');
      Polyglot.validate(readFileSync(process.argv[1]), { formats: process.argv.slice(2) })
        .then((r) => console.log(JSON.stringify(r)))
        .catch((e) => { console.error(e); process.exitCode = 1; });
    """
    result = subprocess.run(['node', '-e', script, str(path), *formats], cwd=ROOT,
                            capture_output=True, text=True, check=True, timeout=30)
    return json.loads(result.stdout)


def main():
    png = (FIXTURES / 'red.png').read_bytes()
    gif = (FIXTURES / 'black.gif').read_bytes()
    ts = (FIXTURES / 'red.ts').read_bytes()
    comment = b'\x21\xfe' + b''.join(
        bytes([len(ts[i:i + 255])]) + ts[i:i + 255]
        for i in range(0, len(ts), 255)
    ) + b'\x00'
    cases = [
        ('png-append', png + ts, 'png', 'png_pipe', bytes([255, 0, 0]) * 4),
        ('gif-append', gif + ts, 'gif', 'gif', bytes([0, 0, 0])),
        ('png-ancillary', png[:-12] + chunk(b'paTs', ts) + png[-12:], 'png', 'png_pipe', bytes([255, 0, 0]) * 4),
        ('gif-comment', gif[:-1] + comment + gif[-1:], 'gif', 'gif', bytes([0, 0, 0])),
    ]
    version = subprocess.run(['ffmpeg', '-version'], capture_output=True, text=True, check=True).stdout.splitlines()[0]
    results = {'ffmpeg': version, 'cases': []}
    with tempfile.TemporaryDirectory(prefix='polyglot-feasibility-') as temporary:
        for name, data, fmt, demuxer, image in cases:
            path = Path(temporary) / name
            path.write_bytes(data)
            result = {
                'name': name, 'sizeBytes': len(data),
                'native': native(path, [fmt, 'mpeg-ts']),
                'image': decode(path, demuxer, image),
                'transport': decode(path, 'mpegts', image),
                'automatic': decode(path, None, image),
            }
            # Optional second image parser, independent of both Rust and FFmpeg.
            tool = 'pngtopam' if fmt == 'png' else 'giftopnm'
            if shutil.which(tool):
                parsed = subprocess.run([tool, str(path)], capture_output=True, timeout=30)
                result[tool] = {'exit': parsed.returncode, 'outputBytes': len(parsed.stdout)}
            results['cases'].append(result)
    print(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()
