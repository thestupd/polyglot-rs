"""Regenerate tiny fixtures; requires Python 3 and FFmpeg on PATH."""
from pathlib import Path
import struct
import subprocess
import zlib

destination = Path(__file__).resolve().parent


def chunk(name, data):
    return (
        struct.pack('>I', len(data)) + name + data
        + struct.pack('>I', zlib.crc32(name + data))
    )


(destination / 'red.png').write_bytes(
    b'\x89PNG\r\n\x1a\n'
    + chunk(b'IHDR', struct.pack('>IIBBBBB', 2, 2, 8, 2, 0, 0, 0))
    + chunk(b'IDAT', zlib.compress((b'\0' + b'\xff\0\0' * 2) * 2))
    + chunk(b'IEND', b'')
)
(destination / 'black.gif').write_bytes(
    b'GIF89a' + struct.pack('<HHBBB', 1, 1, 0x80, 0, 0)
    + b'\0\0\0\xff\xff\xff' + b'\x2c'
    + struct.pack('<HHHHB', 0, 0, 1, 1, 0)
    + b'\x02\x02\x44\x01\x00\x3b'
)
subprocess.run([
    'ffmpeg', '-hide_banner', '-loglevel', 'error', '-f', 'lavfi',
    '-i', 'color=c=red:s=16x16:r=5', '-t', '2', '-c:v', 'mpeg2video',
    '-threads', '1', '-f', 'mpegts', '-y', str(destination / 'red.ts'),
], check=True)
