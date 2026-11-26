# Test fixtures

All fixtures are generated for this repository, contain no third-party content, and use the repository's MIT license.

| File | Content | Verification |
| --- | --- | --- |
| `red.png` | Static 2 × 2 RGB PNG; every pixel is `(255, 0, 0)` | Rust `png` decoder and independent FFmpeg decoding, exact RGB bytes |
| `black.gif` | GIF89a, one 1 × 1 black frame, two-entry palette | Rust `gif` decoder and independent FFmpeg decoding, exact RGB bytes |
| `red.jpg` | Baseline JPEG encoded from `red.png` | Rust structural JPEG handler; used as a carrier fixture |
| `red.ts` | Ten 16 × 16 red MPEG-2 video frames at 5 fps in an MPEG-TS container | Native packet/PSI checks, independent FFmpeg demuxing and decoding; ten frames and near-red RGB values |

Run `python3 tests/fixtures/generate.py` from the repository root with Python and FFmpeg installed to regenerate them. PNG bytes may vary with the zlib version; transport timestamps and encoded media may vary with FFmpeg versions. Pixel/frame assertions specify the reproducible content rather than requiring byte-identical encoder output across versions. The generator uses no external inputs and the fixtures are included so ordinary unit tests do not require these tools.

These are single-format baseline files, not polyglots. Future composition tests must additionally verify every generated interpretation and preserve decoded content.
