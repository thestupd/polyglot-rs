# polyglot-rs

Rust-powered binary inspection, validation and composition candidates with a typed, asynchronous Node.js API.

This repository implements **Milestones 1–3**. It recognizes PNG, GIF and MPEG-TS from their bytes, validates each requested interpretation independently, and provides an extensible composition engine. The Rust core also works without Node.js.

**Carrier strategies: `png`, `gif` and `jpeg`.** Each carries an arbitrary payload inside an image. The output opens as the image; the payload is stored byte-for-byte, never parsed, and addressable by a byte range (`length@offset`) reported for the composition — useful, for example, to place a media segment inside an image and reference it by byte range. Only the carrier image is validated; the payload's format is the caller's concern. Generic append stays conditional; embedding and overlap have reserved interfaces but no enabled built-in algorithms.

See the [roadmap and first-release gates](docs/roadmap.md) for the proposed alpha, platform distribution work, verified composition profiles and 1.0 criteria.

Source: [thestupd/polyglot-rs](https://github.com/thestupd/polyglot-rs). Maintainer: Thieu Quan Ngoc ([hi@stupd.dev](mailto:hi@stupd.dev)). Report bugs through [GitHub issues](https://github.com/thestupd/polyglot-rs/issues).

## Installation

To use the library, install it. Nothing else is required — no Rust toolchain, no compiler, no install-time download script:

```sh
npm install polyglot-rs@alpha
```

The package bundles prebuilt native addons for macOS ARM64/x64, Linux ARM64/x64 (glibc) and Windows x64; the generated napi-rs loader picks the right one at runtime. Requires Node.js 22.13+ (22 or 24 recommended).

## Building from source

Building the repository (not needed to use the package) requires Node.js 22.13+, a current stable Rust toolchain (minimum 1.88) and a platform C/C++ linker. On macOS, install Xcode Command Line Tools; on Windows, use MSVC Build Tools; on Linux, install your distribution's build tools.

```sh
npm ci
npm run build
npm test
```

`npm run build:debug` builds a faster development addon. Builds produce `packages/node/dist/` and `packages/node/native/`; both are ignored by Git. No global npm tool installation is needed. Both `Cargo.lock` and `package-lock.json` are included for reproducible dependency resolution.

Bundling all five binaries keeps the first release simple at the cost of downloading binaries for other platforms. Optional per-platform packages can follow later. The release is tested on macOS 15, Ubuntu 24.04 (glibc 2.39), and Windows Server 2022 with Node.js 22/24. macOS binaries target macOS 15 or later. Older Linux glibc versions, musl, and other targets are outside this alpha's tested support matrix. See [release instructions](docs/releasing.md) for exact artifact verification and publishing steps. A prepared version is not a publication until the registry upload and integrity check succeed.

For a host-only development package, build locally and use `npm run test:package` to exercise a temporary installation. Source-workspace `npm pack` / `npm publish` is guarded so it cannot accidentally upload a package containing only the build host's binary.

## API

CommonJS (`require('polyglot-rs')`) and ESM named imports are supported. Reports are plain objects, and input accepts Node.js `Buffer` or a `Uint8Array`, including subarrays. SharedArrayBuffer-backed inputs are rejected.

### Inspect

```ts
import { Polyglot } from 'polyglot-rs';
import { readFile } from 'node:fs/promises';

const data = await readFile('image.png');
const inspection = await Polyglot.inspect(data);
console.log(inspection.formats); // ['png']
console.log(inspection.metadata.png?.metadata); // dimensions, bitDepth, colorType, chunks
console.log(inspection.metadata.png?.validation);
```

`inspect(data, options?)` returns `Promise<InspectionResult>` with `formats`, `metadata` keyed by format name, and `sizeBytes`. Each metadata entry has `format`, `source`, `metadata`, and `validation`. Inspection runs every registered detector, rather than stopping at the first match.

**Detection is a candidate match.** A PNG signature followed by a truncated header still appears in `formats`, with `validation.valid === false` and an error. Unknown data returns an empty format list. Detection only recognizes streams beginning at byte zero: it does not scan arbitrary embedded offsets. MPEG-TS detection requires three sync bytes spaced 188 bytes apart; explicit validation also accepts shorter complete transport streams.

### Validate

```ts
const reports = await Polyglot.validate(data, {
  formats: ['png', 'gif', 'mpeg-ts', 'custom-binary'],
});
for (const report of reports) {
  console.log(report.format, report.valid, report.errors, report.warnings);
}
```

`validate(data, { formats, ...limits })` returns `Promise<ValidationResult[]>` in the requested order, including duplicates. Request between 1 and 256 interpretations. Each result contains `valid`, `format`, `errors` and `warnings`. Malformed files return invalid reports; argument errors, unavailable native bindings and size limits produce `PolyglotError`. A format without a registered validator returns an invalid report explicitly explaining that no native validator exists. Opaque binary inputs remain representable in the Rust and TypeScript input models.

```ts
import { PolyglotError } from 'polyglot-rs';

try {
  await Polyglot.inspect(data, { maxInputBytes: 1024 });
} catch (error) {
  if (error instanceof PolyglotError) console.error(error.code, error.message);
}
```

Error codes: `ERR_INVALID_INPUT`, `ERR_LIMIT_EXCEEDED`, `ERR_DUPLICATE_FORMAT`, `ERR_DUPLICATE_STRATEGY`, `ERR_UNKNOWN_STRATEGY`, `ERR_UNSUPPORTED_COMBINATION`, `ERR_INVALID_COMPOSITION`, `ERR_NATIVE_UNAVAILABLE`, `ERR_NATIVE_FAILURE`, `ERR_CUSTOM_DETECTION`. Decoder resource failures may instead produce an invalid report when the decoder cannot distinguish a limit failure from malformed data.

### Check compatibility

```ts
const compatibility = await Polyglot.checkCompatibility({
  formats: ['png', 'mpeg-ts'],
  strategy: 'auto', // or an explicitly registered native strategy name
});
console.log(compatibility.status); // 'supported' through the png carrier strategy
console.log(compatibility.strategies, compatibility.limitations);
console.log(Polyglot.listStrategies()); // append, png, gif, jpeg, embed, overlap
```

This query takes between 2 and 256 format names, preserves their input order and does not inspect buffers. `unsupported` means no selected strategy works. `conditional` means an assembly may be attempted subject to the listed limitations. `supported` is returned by the `png`, `gif` and `jpeg` carrier strategies: the first input is that image format and the second is any other format. Unknown names (e.g. `zip`) with no carrier are treated as opaque and get a conditional append report. `strategies` lists candidate algorithms; `limitations` explains the constraints.

### Carry a payload in an image (`png`, `gif`, `jpeg`)

```ts
const image = await readFile('poster.gif'); // a .png, .gif or .jpg
const payload = await readFile('payload.bin'); // any bytes, in any format
const result = await Polyglot.create({
  inputs: [{ format: 'gif', data: image }, { format: 'payload', data: payload }],
});
await writeFile('out.gif', result.toBuffer());
console.log(result.report.strategy); // 'gif' (the carrier)
console.log(result.byteRange()); // e.g. '6392@35' → length@offset of the payload
```

The first input is the carrier image and is validated as that image; the second input is the payload, stored byte-for-byte and never parsed. Its format name is arbitrary. `auto` picks the carrier strategy for a `png`/`gif`/`jpeg` first input. [`examples/embed.ts`](examples/embed.ts) runs this end to end.

**Carrier image omitted.** Pass a single input (the payload) and name the carrier through `strategy` to get a built-in 1×1 image; `auto` with a single input defaults to a `png` carrier:

```ts
const result = await Polyglot.create({ inputs: [{ format: 'payload', data: payload }], strategy: 'jpeg' });
```

| Strategy | Layout | Carrier image | Payload offset |
| --- | --- | --- | --- |
| `png` | Payload stored in a private ancillary chunk (`paTs`) right after IHDR | The whole file remains a strictly valid PNG; decoders skip the chunk | `41` |
| `gif` | Payload appended after the GIF trailer (`0x3B`) | Decoders stop at the trailer; strict validators that reject trailing bytes will not accept the whole file | `<gif size>` |
| `jpeg` | Payload appended after the JPEG EOI marker (`FFD9`) | Decoders stop at EOI; strict validators that reject trailing bytes will not accept the whole file | `<jpeg size>` |

`result.report.regions` lists where each interpretation lives (carrier and payload). `result.region(format)` returns one region, `result.payloadRegion()` returns the carried payload region, and `result.byteRange()` formats it as `length@offset`. The report's `validation` covers only the carrier; the payload input's validation is `null` and a warning records that it was carried unchanged.

### Create a candidate

```ts
// A transport stream can be losslessly reassembled from packet-aligned pieces.
// This is an assembly example, not a multi-format polyglot.
const transport = await readFile('video.ts');
const split = 188 * 4;
const result = await Polyglot.create({
  inputs: [
    { format: 'mpeg-ts', data: transport.subarray(0, split), label: 'first' },
    { format: 'mpeg-ts', data: transport.subarray(split), label: 'rest' },
  ],
  strategy: 'auto',
});
const output = result.toBuffer(); // owned Buffer copy
console.log(result.report.validated, result.report.compatibility.status);
```

`create({ inputs, strategy?, validate?, maxInputBytes?, maxDecodedBytes?, maxOutputBytes? })` returns `Promise<PolyglotResult>`. Supply between 2 and 256 inputs. By default, every input is validated against its own declared format and the complete candidate is independently checked against every distinct requested format. Any failed native check rejects the result with `ERR_INVALID_COMPOSITION`. The report includes the chosen strategy, compatibility assessment, input sizes/labels/validation, output validation results, byte size, and warnings. `validated: true` means native structural validation passed; it does not mean external parser/decoded-content verification passed. Compatibility is never automatically promoted after native validation.

`auto` tries supported strategies before conditional strategies, preserving registration order within each status. It tries the next candidate after a strategy or output validation rejects an invalid/unsupported composition. Argument, resource and other operational errors stop the attempt. Explicit selection tries only that strategy. The built-in order is append, png, gif, jpeg, embed, overlap. Missing names produce `ERR_UNKNOWN_STRATEGY`.

Opaque inputs are accepted as **explicitly unvalidated** assemblies:

```ts
const candidate = await Polyglot.create({
  inputs: [
    { format: 'opaque-a', data: Buffer.from([0, 255]) },
    { format: 'opaque-b', data: new Uint8Array([42]) },
  ],
  strategy: 'append',
  validate: false,
});
console.log(candidate.report.validated); // false
console.log(candidate.report.compatibility.status); // conditional
```

When a strategy declares `regions`, each format is validated over its own region instead of the complete output. `validate: false` skips input/output validation and leaves validation reports empty/null. It does not bypass a strategy's unsupported layout or alignment constraints. Such output is a binary candidate, not a verified polyglot. JavaScript detection hooks cannot enable native composition or supply validators. `PolyglotResult.sizeBytes` reports the buffer length, and `toBuffer()` returns a fresh owned copy each time.

### Custom detection

```ts
import { Polyglot, PolyglotEngine } from 'polyglot-rs';

Polyglot.registerFormat({
  name: 'custom-format',
  detect: (buffer) => buffer.subarray(0, 4).equals(Buffer.from('CSTM')),
});

// Or keep registrations scoped to your application/component:
const engine = new PolyglotEngine();
engine.registerFormat({ name: 'opaque', detect: async () => false });
engine.unregisterFormat('opaque');
```

Names use lowercase ASCII letters, digits and hyphens. Duplicate names and attempts to replace native handlers are rejected. Detection hooks may return `boolean` or `Promise<boolean>`. They execute in JavaScript and receive isolated copies. They **do not provide native validation, parsing, embedding or composition**. Their inspection entries use `source: 'javascript'` and `validation.valid: false`. Hooks are application code and may block the event loop if they do expensive synchronous work.

## Supported formats and validation scope

| Handler | Detection | Validation | Metadata |
| --- | --- | --- | --- |
| PNG | 8-byte signature | Chunk bounds, types, critical ordering, CRCs, IHDR, palette, IDAT/IEND, static-image decompression and filtering through the Rust `png` decoder | Width, height, bit depth, color type, chunk count |
| GIF | GIF87a / GIF89a | Screen/frame bounds, color tables, extension envelopes, sub-block bounds, trailer, LZW decoding and palette indices through the Rust `gif` decoder | Width, height, frame count, version |
| MPEG-TS | Three 188-byte sync positions | Complete 188-byte packet alignment, headers, adaptation bounds, PCR encoding, stuffing, continuity, PAT/PMT reassembly, section CRCs and descriptor bounds | Packet size/count, PIDs, program numbers, stream PIDs, PAT presence |
| JPEG | `FFD8FF` start | Marker walk, segment lengths, one SOF with valid dimensions and component count, a scan, EOI last, no trailing bytes (structural only; pixel data is not decoded) | Width, height, components, segment count |

PNG ancillary metadata semantics are not exhaustively validated. APNG is explicitly rejected until animation validation is implemented. GIF requires at least one raster frame and an in-file palette; text-only streams and externally supplied palettes are outside the current profile. GIF plain text rendering and application extension semantics are not validated. PNG and GIF reject bytes after their terminators; this is a strict validation policy, not a claim about permissive external parsers.

MPEG-TS validates **transport structure**, not codec decodability. Null-only streams and partial captures without complete PAT/PMT tables can be transport-valid and receive warnings. Scrambled payloads, PES structure, media codec data, timing, table-version transitions and adaptation-extension semantics are not validated. M2TS (192-byte) and 204-byte packet variants are outside the current handler. A `valid: true` transport report does not prove that a player can decode the content.

ZIP, MP4/fMP4 and PDF are planned and currently have no handlers. JPEG is validated structurally (markers and frame header), not pixel-decoded, unlike PNG and GIF.

## Composition status

| Category | Current status |
| --- | --- |
| Supported inspection handlers | PNG, GIF, MPEG-TS |
| Implemented composition strategies | `png`, `gif`, `jpeg` image carriers, conditional append; native embedding capability and reserved embed/overlap interfaces |
| Payload carriers | PNG, GIF and JPEG images carry an arbitrary unvalidated payload addressable by byte range; only the carrier image is validated |
| Experimental research | GIF comment insertion breaks TS; whole-file segment playback without a byte range depends on the player |
| Further investigation | Additional carrier formats; fuller JPEG validation; verified multi-format decode profiles |

Carrier strategies store the payload unchanged and validate only the carrier image; they do not claim the payload decodes.

## Architecture

```text
crates/polyglot-core     Input model, errors, limits, handlers/capabilities, registry, composer/contracts
crates/polyglot-formats  PNG, GIF, MPEG-TS handlers and built-in registry
crates/polyglot-strategies png/gif/jpeg image carriers, conditional append; reserved embed/overlap interfaces
crates/polyglot-napi     Private napi-rs bridge, libuv asynchronous worker tasks
packages/node/src       Public TypeScript types, input checks, errors, detection hooks
tests/fixtures          Small reproducible images and MPEG-2 transport stream
tests/integration       Node API and native binding checks
docs/                   Feasibility report and observed parser results
```

Add a Rust format by implementing `polyglot_core::FormatHandler` and registering it in a `Registry`. Handlers implement `name`, `detect`, `validate` and `inspect`; limit-aware methods allow them to enforce decoding budgets. Native handlers must obey the configured limits, return deterministic reports and safely handle malformed input. The core has no dependency on any format crate or Node.js.

Optional `AppendCapability` and `EmbeddingCapability` interfaces describe layout constraints and native embedding locations/operations. PNG/GIF prohibit both prefixes and suffixes under their strict profiles; MPEG-TS requires 188-byte alignment and complete-output validation. Unknown handlers without capabilities remain unproven. Append constraints are necessary conditions, not proof that neighboring bytes will parse.

Add a Rust algorithm by implementing `CompositionStrategy` (`name`, `check_compatibility`, `compose`, optional `requirements` / `check_formats`) and calling `Composer::register_strategy`. Format-only queries use `check_formats`, whose default conservatively requires input bytes; they never pass fabricated empty inputs to a custom byte-aware checker. Return `CompositionResult::candidate` (optionally `.with_regions(...)` to say where each interpretation lives); Composer reconstructs the final report and performs native validation rather than trusting a strategy's prefilled validation claims. Native strategy authors must enforce allocation budgets before allocating output and may claim `supported` only with documented independent parser/content evidence. Overlap algorithms implement this same interface with their format-specific constraints. Custom native handlers/algorithms require rebuilding the bridge; no arbitrary JavaScript-to-Rust composition bridge is implied.

The N-API boundary snapshots each input once before scheduling a libuv worker. This bounded copy protects Rust reads from concurrent JavaScript mutation. It does not expose borrowed JavaScript bytes to a worker. Large binary processing occurs in Rust; small reports cross back as JSON and composed bytes return as a native Buffer. `toBuffer()` copies that output for caller ownership. Custom JavaScript hooks use additional copies for isolation. Native async tasks share Node's libuv worker pool.

## Limits and safety

Inspection, validation and creation accept `maxInputBytes` and `maxDecodedBytes`, defaulting to 64 MiB each. Creation applies the input budget to the sum of all inputs before any native snapshots and also has a separate `maxOutputBytes` budget, default 64 MiB. Values must be positive integers no greater than 2^32 − 1 in Node. PNG checks decoded image size and uses row decoding; GIF sums decoded indexed pixels across frames before decoding. Rust callers use `Limits` and `CompositionOptions`. Per-handler limits do not cap process-wide memory or concurrent operations; applications should bound their concurrency. Decoder-internal memory limits are best-effort limits in addition to the explicit decoded-byte checks.

Binary offsets use checked cursors or bounded packet/section slices. PSI sections are capped at 1024 bytes. Parsers never execute embedded bytes or change source buffers. Rust parser crates forbid unsafe code; napi-rs owns the FFI boundary.

## Development and testing

```sh
npm run build:debug
npm run typecheck
npm run lint
npm test
npm run test:package       # Pack, install into an isolated directory, load and validate
```

Fixture generation is documented in [tests/fixtures/README.md](tests/fixtures/README.md).

Rust tests cover headers, checksums, malformed lengths/offsets, every image truncation, deterministic random input and byte mutations, transport synchronization/continuity, registry behavior, strategy selection/fallback, custom native formats, aggregate budgets and whole-output validation. Node tests cover Buffer/Uint8Array views, snapshot behavior, compatibility, candidate reports, independent validation, hooks, typed errors and module imports. Rust-only usage: `cargo test -p polyglot-core -p polyglot-formats -p polyglot-strategies`.

The release workflow builds and tests macOS ARM64/x64, Linux ARM64/x64 (glibc) and Windows x64, verifies common commit/version and artifact hashes, then assembles one tarball. Ten consumer jobs install that same tarball on Node.js 22/24 across the five targets. Publication uses the tested tarball, skips lifecycle scripts, selects the `alpha` tag explicitly, and verifies registry integrity. Remote run results remain the evidence for each version; workflow configuration alone does not establish passing jobs.

## References

- [PNG specification](https://www.w3.org/TR/png/)
- [GIF89a specification](https://www.w3.org/Graphics/GIF/spec-gif89a.txt) — Graphics Interchange Format is a service mark of CompuServe Incorporated.
- [FFmpeg MPEG-TS demuxer](https://ffmpeg.org/doxygen/trunk/mpegts_8c_source.html)
- [napi-rs asynchronous tasks](https://napi.rs/docs/concepts/async-task)

MIT licensed. Third-party decoders retain their respective licenses.
