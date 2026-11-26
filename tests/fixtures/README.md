# Test fixtures

All fixtures are generated for this repository, contain no third-party content, and use the repository's MIT license.

| File | Content | Verification |
| --- | --- | --- |
| `red.png` | Static 2 × 2 RGB PNG; every pixel is `(255, 0, 0)` | Rust `png` decoder, exact RGB bytes |
| `black.gif` | GIF89a, one 1 × 1 black frame, two-entry palette | Rust `gif` decoder, exact RGB bytes |
| `red.jpg` | Baseline JPEG encoded from `red.png` | Rust structural JPEG handler; used as a carrier fixture |
| `red.ts` | Ten 16 × 16 red MPEG-2 video frames at 5 fps in an MPEG-TS container | Native packet/PSI checks; also used as a carrier payload fixture |

Run `python3 tests/fixtures/generate.py` from the repository root to regenerate them; it needs Python and, for `red.ts` and `red.jpg`, FFmpeg. `red.png` and `black.gif` are written directly with no external tool. Encoded media may vary with FFmpeg/zlib versions, so tests assert reproducible content rather than byte-identical encoder output. The fixtures are committed so ordinary unit tests need neither Python nor FFmpeg.

These are single-format baseline files, not polyglots. The carrier strategies combine an image fixture with a payload (for example `red.ts`) at test time.
