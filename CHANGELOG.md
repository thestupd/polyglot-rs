# Changelog

## Unreleased

### Features

- Image carrier strategies `png`, `gif` and `jpeg`: carry an arbitrary payload inside an image. Only the carrier image is validated; the payload is stored byte-for-byte and never parsed. `png` uses a private ancillary chunk (`paTs`) after IHDR (the whole file stays a valid PNG); `gif` and `jpeg` append the payload after the trailer / EOI marker.
- Pass a single input (the payload) with `strategy: 'png' | 'gif' | 'jpeg'` (or `auto`, which defaults to `png`) to use a built-in 1×1 carrier image.
- JPEG inspection/validation handler (structural: markers, frame header, scan, EOI; pixel data is not decoded).
- Composition reports list `regions` (format, offset, length); each validated format is checked within its region, and carried payloads are skipped.
- `PolyglotResult.region(format)`, `PolyglotResult.payloadRegion()` and `PolyglotResult.byteRange()` (`"<length>@<offset>"`).
- `CARRIER_FORMATS`, `defaultCarrier()` and the `CarrierFormat` type are exported.
- `examples/embed.ts` carries a payload into an image.

### Changed

- Removed the FFmpeg-based parser test suite; verification no longer depends on FFmpeg.

## 0.1.0-alpha.1

First public alpha. The API and validation profiles may change before 1.0.

- Independent Rust core with extensible format handlers, registry and composition strategies.
- PNG, GIF and 188-byte MPEG-TS inspection and native validation.
- Typed asynchronous Node.js API with Buffer and Uint8Array input support, CommonJS and ESM imports.
- Compatibility reports, conditional append, composition reports and explicit unvalidated opaque assembly.
- Input/output budgets, checked binary parsing and independent decoding tests.
- Five-platform native release bundle: macOS ARM64/x64, Linux ARM64/x64 with glibc, and Windows x64.

No production multi-format polyglot combination is enabled. Embedding/overlap algorithms remain reserved interfaces; research observations are documented separately. MPEG-TS native validation checks transport structure and does not prove codec decodability.
