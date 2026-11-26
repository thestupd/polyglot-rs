# polyglot-rs

Rust-powered binary inspection, validation and composition candidates with a typed, asynchronous Node.js API.

This repository implements **Milestones 1–3**. It recognizes PNG, GIF and MPEG-TS from their bytes, validates each requested interpretation independently, and provides an extensible composition engine. The Rust core also works without Node.js.

**No production polyglot combinations are supported or claimed yet.** `create()` assembles candidates with explicit validation reports. Generic append is conditional; embedding and overlap have reserved interfaces but no enabled built-in algorithms. The [feasibility report](docs/feasibility.md) records parser/decoding experiments before Milestone 4.

See the [roadmap and first-release gates](docs/roadmap.md) for the proposed alpha, platform distribution work, verified composition profiles and 1.0 criteria.

Source: [thestupd/polyglot-rs](https://github.com/thestupd/polyglot-rs). Maintainer: Thieu Quan Ngoc ([hi@stupd.dev](mailto:hi@stupd.dev)). Report bugs through [GitHub issues](https://github.com/thestupd/polyglot-rs/issues).

## Installation and build

Requires Node.js 22.13+ (22 or 24 recommended), a current stable Rust toolchain (minimum 1.88), and a platform C/C++ linker. On macOS, install Xcode Command Line Tools; on Windows, use MSVC Build Tools; on Linux, install your distribution's build tools.

```sh
npm ci
npm run build
npm test
```

`npm run build:debug` builds a faster development addon. Builds produce `packages/node/dist/` and `packages/node/native/`; both are ignored by Git. No global npm tool installation is needed. Both `Cargo.lock` and `package-lock.json` are included for reproducible dependency resolution.

The first public release is the alpha channel:

```sh
npm install polyglot-rs@alpha
```

The public alpha bundles native addons for macOS ARM64/x64, Linux ARM64/x64 with glibc, and Windows x64 in one package. The generated napi-rs loader selects the local binary; consumers do not need Rust or an install-time binary download script. Bundling all five binaries keeps the first release simple at the cost of downloading binaries for other platforms. Optional per-platform packages can follow later.

The release is tested on macOS 15, Ubuntu 24.04 (glibc 2.39), and Windows Server 2022 with Node.js 22/24. macOS binaries target macOS 15 or later. Older Linux glibc versions, musl, and other targets are outside this alpha's tested support matrix. See [release instructions](docs/releasing.md) for exact artifact verification and publishing steps. A prepared version is not a publication until the registry upload and integrity check succeed.

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
console.log(compatibility.status); // 'unsupported' under current strict profiles
console.log(compatibility.strategies, compatibility.limitations);
console.log(Polyglot.listStrategies()); // requirements for append, embed, overlap
```

This query takes between 2 and 256 format names, preserves their input order and does not inspect buffers. `unsupported` means no selected strategy works under the current native parser profiles. `conditional` means an assembly may be attempted subject to the listed limitations; it is not a claim that the pair decodes. `supported` is reserved for native strategy implementations whose documented independent parser and content tests establish their stated guarantees. No built-in strategy returns `supported` yet.

Unknown names, including currently unimplemented formats such as JPEG/ZIP, are treated as opaque: a conditional append report for them says only that bytes can be assembled without a native validator. `strategies` lists candidate algorithms; `limitations` explains why their output is not yet verified support.

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

`auto` tries supported strategies before conditional strategies, preserving registration order within each status. It tries the next candidate after a strategy or output validation rejects an invalid/unsupported composition. Argument, resource and other operational errors stop the attempt. Explicit selection tries only that strategy. The built-in order is append, embed, overlap. Missing names produce `ERR_UNKNOWN_STRATEGY`.

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

`validate: false` skips input/output validation and leaves validation reports empty/null. It does not bypass a strategy's unsupported layout or alignment constraints. Such output is a binary candidate, not a verified polyglot. JavaScript detection hooks cannot enable native composition or supply validators. `PolyglotResult.sizeBytes` reports the buffer length, and `toBuffer()` returns a fresh owned copy each time.

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

PNG ancillary metadata semantics are not exhaustively validated. APNG is explicitly rejected until animation validation is implemented. GIF requires at least one raster frame and an in-file palette; text-only streams and externally supplied palettes are outside the current profile. GIF plain text rendering and application extension semantics are not validated. PNG and GIF reject bytes after their terminators; this is a strict validation policy, not a claim about permissive external parsers.

MPEG-TS validates **transport structure**, not codec decodability. Null-only streams and partial captures without complete PAT/PMT tables can be transport-valid and receive warnings. Scrambled payloads, PES structure, media codec data, timing, table-version transitions and adaptation-extension semantics are not validated. M2TS (192-byte) and 204-byte packet variants are outside the current handler. A `valid: true` transport report does not prove that a player can decode the content.

JPEG, ZIP, MP4/fMP4 and PDF are planned and currently have no handlers.

## Composition status

| Category | Current status |
| --- | --- |
| Supported inspection handlers | PNG, GIF, MPEG-TS |
| Implemented composition strategies | Conditional append; native embedding capability and reserved embed/overlap interfaces |
| Verified polyglot combinations | None |
| Experimental research | PNG ancillary + TS decodes in FFmpeg with TS resynchronization; GIF comment insertion breaks TS; neither is enabled in the engine |
| Further investigation | JPEG + ZIP; a packet-aware GIF/TS layout; a named tolerant PNG/TS parser profile |

The [feasibility report](docs/feasibility.md) includes reproducible full-decoding experiments and explains the difference between strict structure validation and tolerant demuxer behavior. Milestone 4 must introduce an explicit parser profile and additional independent decoding/content tests before enabling a production combination. Concatenation and format probing alone do not establish support.

## Architecture

```text
crates/polyglot-core     Input model, errors, limits, handlers/capabilities, registry, composer/contracts
crates/polyglot-formats  PNG, GIF, MPEG-TS handlers and built-in registry
crates/polyglot-strategies Conditional append; reserved embed/overlap strategy interfaces
crates/polyglot-napi     Private napi-rs bridge, libuv asynchronous worker tasks
packages/node/src       Public TypeScript types, input checks, errors, detection hooks
tests/fixtures          Small reproducible images and MPEG-2 transport stream
tests/integration       Node API and native binding checks
tests/parsers           Independent FFmpeg decoding with content assertions
docs/                   Feasibility report and observed parser results
```

Add a Rust format by implementing `polyglot_core::FormatHandler` and registering it in a `Registry`. Handlers implement `name`, `detect`, `validate` and `inspect`; limit-aware methods allow them to enforce decoding budgets. Native handlers must obey the configured limits, return deterministic reports and safely handle malformed input. The core has no dependency on any format crate or Node.js.

Optional `AppendCapability` and `EmbeddingCapability` interfaces describe layout constraints and native embedding locations/operations. PNG/GIF prohibit both prefixes and suffixes under their strict profiles; MPEG-TS requires 188-byte alignment and complete-output validation. Unknown handlers without capabilities remain unproven. Append constraints are necessary conditions, not proof that neighboring bytes will parse.

Add a Rust algorithm by implementing `CompositionStrategy` (`name`, `check_compatibility`, `compose`, optional `requirements` / `check_formats`) and calling `Composer::register_strategy`. Format-only queries use `check_formats`, whose default conservatively requires input bytes; they never pass fabricated empty inputs to a custom byte-aware checker. Return `CompositionResult::candidate`; Composer reconstructs the final report and performs native validation rather than trusting a strategy's prefilled validation claims. Native strategy authors must enforce allocation budgets before allocating output and may claim `supported` only with documented independent parser/content evidence. Overlap algorithms implement this same interface with their format-specific constraints. Custom native handlers/algorithms require rebuilding the bridge; no arbitrary JavaScript-to-Rust composition bridge is implied.

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
npm run test:parsers       # FFmpeg tests; explicitly skipped if unavailable
npm run test:package       # Pack, install into an isolated directory, load and validate
python3 scripts/feasibility.py # Research controls; requires FFmpeg and a built addon
```

Use `REQUIRE_EXTERNAL_PARSERS=1 npm run test:parsers` to require FFmpeg, or set `FFMPEG_PATH` to its executable. Independent tests assert exact PNG/GIF pixels and decoded MPEG-2 frame counts/colors, including the engine's lossless TS reassembly. They exercise baseline fixtures and transport assembly; there are no production polyglot outputs yet. Fixture generation is documented in [tests/fixtures/README.md](tests/fixtures/README.md).

Rust tests cover headers, checksums, malformed lengths/offsets, every image truncation, deterministic random input and byte mutations, transport synchronization/continuity, registry behavior, strategy selection/fallback, custom native formats, aggregate budgets and whole-output validation. Node tests cover Buffer/Uint8Array views, snapshot behavior, compatibility, candidate reports, independent validation, hooks, typed errors and module imports. Rust-only usage: `cargo test -p polyglot-core -p polyglot-formats -p polyglot-strategies`.

The release workflow builds and tests macOS ARM64/x64, Linux ARM64/x64 (glibc) and Windows x64, verifies common commit/version and artifact hashes, then assembles one tarball. Ten consumer jobs install that same tarball on Node.js 22/24 across the five targets. Linux builds require independent FFmpeg decoding. Publication uses the tested tarball, skips lifecycle scripts, selects the `alpha` tag explicitly, and verifies registry integrity. Remote run results remain the evidence for each version; workflow configuration alone does not establish passing jobs.

## References

- [PNG specification](https://www.w3.org/TR/png/)
- [GIF89a specification](https://www.w3.org/Graphics/GIF/spec-gif89a.txt) — Graphics Interchange Format is a service mark of CompuServe Incorporated.
- [FFmpeg MPEG-TS demuxer](https://ffmpeg.org/doxygen/trunk/mpegts_8c_source.html)
- [napi-rs asynchronous tasks](https://napi.rs/docs/concepts/async-task)

MIT licensed. Third-party decoders retain their respective licenses.
