# polyglot-rs roadmap

Updated: 2026-11-26. Version targets below are proposed release scopes, not delivery dates or support promises.

## Release assessment

The implementation is ready for local evaluation and an engineering preview. It is **not yet ready for a general public npm release or version 1.0**.

The first public release should be **0.1.0-alpha.1**, positioned as a Rust-powered binary inspection and validation library with an experimental composition framework. It can ship without a production polyglot combination if that scope is explicit. The package's current `0.1.0` manifest value does not mean a release has been published.

Before that public alpha, finish platform distribution and obtain passing remote CI results. A host-specific tarball already works locally, but that does not establish that a normal npm installation works across all advertised platforms.

## Current implementation

| Area | Current evidence | Remaining limitation |
| --- | --- | --- |
| Rust architecture | Independent core, format handlers, registry, composer and strategy crate | Public extension contracts remain experimental |
| Node integration | napi-rs workers, typed API, Buffer/Uint8Array support, CommonJS/ESM imports | Five-platform bundle pipeline prepared; remote consumer tests gate publication |
| Inspection and validation | PNG, GIF and 188-byte MPEG-TS handlers | Static PNG profile; documented GIF and transport-validation limits |
| Composition | Conditional append, whole-output validation, reports, native extension interfaces | No production multi-format combination is enabled |
| Research | PNG ancillary/TS decoding experiment; GIF insertion failure reproduced | Parser-specific observations are not general format support |
| Verification | Latest local run: 19 Rust tests, 16 Node tests, 4 release integrity tests and 3 independent decoding tests; release build, lint, type checks and packed-consumer check passed | Remote platform results and registry verification gate each publication |
| Release operations | CI bundle/consumer workflow, checked artifact manifests, guarded alpha publishing command, lockfiles, changelog and documentation | First public upload pending; optional platform child packages are deferred in favor of one bundled package |

The public source repository is [thestupd/polyglot-rs](https://github.com/thestupd/polyglot-rs). The local repository is initialized on `main` with its SSH origin configured. The first source commit/push and actual CI runs are still needed to establish release provenance and platform results.

## Proposed release sequence

| Target | Main outcome | Gate to move forward |
| --- | --- | --- |
| 0.1.0-alpha.1 | Installable inspection/validation preview | Platform packages, passing remote matrix, honest public scope |
| 0.1.0 | Reliable initial npm release for the documented inspection APIs | Alpha feedback addressed; installation and API behavior verified |
| 0.2.0 | First explicitly profiled polyglot strategy | Independent parsing and full decoded-content preservation |
| 0.3.0 | JPEG and ZIP inspection; investigate their composition | Independent image/archive validation and evidence for any enabled pair |
| 0.4.0 | Additional handlers and parser profiles | Each handler/pair meets its own correctness and resource gates |
| 1.0.0-beta | API stabilization and sustained hardening | Frozen contracts, fuzzing coverage, performance baseline, upgrade guidance |
| 1.0.0 | Stable, documented general-purpose foundation | Proven release pipeline, stable API, at least one verified composition profile |

Release numbering can change with scope. A release may support more inspection formats than composition combinations. Adding a handler does not automatically enable any pair involving that format.

## First public alpha

Prioritize this work before adding more format combinations. It completes the distribution part of the original specification early enough for real users to test the existing APIs.

### Distribution and platform gates

- [x] Keep the npm package name `polyglot-rs` and set the repository, homepage, issue tracker and maintainer metadata.
- [ ] Verify ownership/availability of the root and platform package names on the public npm registry.
- [x] Initialize the local Git repository and configure `git@github.per:thestupd/polyglot-rs.git` as origin.
- [ ] Commit and push the source, and run CI.
- [ ] Build macOS ARM64/x64, Linux ARM64/x64 with glibc, and Windows x64 binaries.
- [ ] Define minimum macOS/Windows versions and a Linux glibc baseline. Choose build environments that actually satisfy those runtime baselines; successful builds on a recent Linux runner alone do not prove older-system compatibility.
- [x] Implement one tarball containing the five binaries, selected by the generated napi-rs loader; defer optional platform child packages.
- [ ] Test the assembled root package and its selected platform package on every advertised target using Node.js 22 and 24, without Rust or a native build step in the consumer environment.
- [ ] Test a missing/unsupported native binary and provide an actionable load error. State that musl and other unbuilt targets are outside the initial support matrix.
- [x] Make the release pack step consume the exact tested artifacts. Source-workspace packing/publishing is guarded and release packaging never rebuilds native binaries.
- [ ] Obtain passing remote build, lint, type, Rust, Node, decoding and packed-consumer jobs. Link those runs to the release candidate.

### Public contract and release gates

- [ ] Review the exported API, errors, size-limit semantics and result ownership as alpha contracts.
- [ ] State prominently that `validated` means native validation, while `supported` requires documented independent parser/content evidence.
- [ ] Explain that opaque append with `validate: false` returns an unvalidated binary assembly, and that JavaScript hooks provide detection only.
- [ ] Describe current strict PNG/GIF terminator behavior, partial-capture MPEG-TS warnings, and unsupported animation/packet variants.
- [ ] Add a changelog, contributor instructions, a vulnerability-reporting contact and a reproducible release procedure.
- [ ] Keep package versions and selected native package versions consistent; configure the intended public registry explicitly in the release workflow.
- [ ] Set up a controlled publishing identity, preferably npm trusted publishing where supported, and record artifact provenance.
- [ ] After the release gates pass, publish the alpha using an explicit `alpha` distribution tag. npm's default publish tag is `latest`; a prerelease version should not be placed there unintentionally. See [npm publish configuration](https://docs.npmjs.com/cli/v11/commands/npm-publish/#tag).

**Alpha exit criterion:** a user on every advertised target can install the package normally and run inspection/validation without installing Rust. The release notes identify composition as experimental and list zero production polyglot combinations.

This roadmap proposes a release process; it does not authorize or perform publication.

## Initial 0.1.0 release

Collect alpha feedback from real consuming projects. Fix installation, declaration/module-resolution and error-reporting problems before expanding the feature list. Maintain the strict default validators and keep the unsupported/conditional distinction visible in examples.

Expand the fixture corpus beyond tiny baseline images and one transport encoding. Cover animated GIFs, palette/local-table cases, interlaced and indexed PNGs, multiple MPEG-TS programs, adaptation fields and malformed section boundaries within the handlers' stated scope. Unsupported cases should fail clearly rather than silently receiving broader guarantees.

**Exit criterion:** no known release-blocking installation or correctness defects within the advertised scope; passing platform consumers; an explicit pre-1.0 compatibility policy and release notes. A production polyglot strategy is not required for this inspection-focused release.

## First production composition profile

Start from the [PNG/GIF + MPEG-TS feasibility report](feasibility.md). PNG ancillary embedding is the leading candidate, but it needs a named transport-resynchronization profile. It is not valid strict byte-zero MPEG-TS merely because a tolerant demuxer decodes it.

- Define explicit parser-profile selection and report which interpretation/profile was validated. Keep strict defaults unchanged.
- Implement bounded stream-offset discovery and describe any bytes skipped by a tolerant profile. Do not hide discarded prefixes/suffixes behind an unqualified `valid` result.
- Implement PNG ancillary embedding in Rust, with checked chunk lengths, CRCs, source preservation and output budgets.
- Independently decode the complete output as an image and as media. Compare image pixels, frame/audio content where applicable, and payload identity rather than relying on probing or stream discovery.
- Test multiple input sizes, metadata layouts, stream lengths and available standard parser versions. Exercise corrupt/truncated candidates and the exact declared resynchronization bounds.
- Ensure `inspect`, `validate`, `checkCompatibility` and `create` agree about profile names and guarantees. Format-only queries must remain conservative where bytes determine feasibility.
- Enable `supported` only for the documented, tested profile. Retain limitations for other parsers and arbitrary inputs.

**Exit criterion:** at least one production strategy produces a structured output that passes both intended interpretations and preserves the tested content. A temporary research file, concatenation control, successful probe or partial decode does not satisfy the gate.

GIF + MPEG-TS stays a separate investigation. Its sub-block framing needs a packet-aware layout; ordinary comment insertion has already failed the transport decoding experiment. It may remain unsupported if its practical parser constraints cannot be met.

## JPEG and ZIP

Implement these handlers before attempting their composition. JPEG needs marker/segment parsing, bounds validation and independent decoding. ZIP needs bounded directory/entry handling, offset validation and applicable checksum checks, with explicit policies for archive variants and decompression limits.

Then investigate JPEG + ZIP under named parser profiles. Preserve decoded image content and extracted archive entries through independent tools. Keep ZIP64, encrypted archives and other variants explicitly unsupported until implemented and tested. Do not infer pair support from the existence of both handlers.

**Exit criterion:** reliable independent inspection first; a composition profile is enabled only if both complete interpretations pass their content checks.

## Additional formats and extensibility

Add MP4/fMP4 and PDF handlers incrementally, with documented validation depth and independent parser tests. Consider WAV and custom native handlers according to concrete consumer needs. MP4 and fragmented MP4 should have explicit, separately tested coverage rather than sharing an unsupported general claim.

Improve native registration examples, capability discovery, stable report schemas and third-party strategy integration. Keep format recognition, format-specific capabilities, strategy selection and parser compatibility separate. An arbitrary JavaScript composition bridge, browser/WASM build, CLI or streaming interface should be a separate design decision with a demonstrated use case.

**Exit criterion:** every new format has malformed/truncated cases and resource-limit coverage; every new composition has independent interpretation/content evidence.

## Hardening and API stabilization

- Add coverage-guided fuzz targets for each parser, registry inspection and composition path, retaining minimized regressions.
- Add property checks for deterministic output, no source mutation, bounded growth and the consistency of reports with actual validation.
- Exercise malicious advertised lengths/dimensions, decompression expansion, large program tables and many concurrent operations.
- Benchmark latency, throughput and peak memory for small and large inputs; document hardware, fixtures, limits and measurement methods before making performance claims.
- Define concurrency and cancellation behavior, distinguishing queued task cancellation from stopping an operation that is already running.
- Review ambiguous validation outcomes and errors, especially resource failures versus malformed data, before freezing the public contract.
- Document deprecation/versioning policy, native handler/strategy obligations and migration examples. Maintain a parser/version compatibility matrix with links to test evidence.

**1.0 gate:** stable TypeScript contracts and error semantics; reliable platform distribution; sustained parser/resource tests; documented performance limits; at least one verified composition profile; and release automation that publishes the exact tested artifacts.

## Package identity

The selected name is **polyglot-rs**. Keep it consistent across npm, native binary naming, documentation and examples.

- Public repository: https://github.com/thestupd/polyglot-rs
- Git origin over the existing SSH alias: `git@github.per:thestupd/polyglot-rs.git`
- Maintainer: Thieu Quan Ngoc, hi@stupd.dev
- Public registry: https://registry.npmjs.org/

Root and platform package availability/ownership must still be verified before publication. Published metadata uses the public HTTPS repository URL; the machine-specific SSH alias belongs only in local Git configuration.

## Immediate priority

Commit and push the repository, verify npm package ownership, and finish the multi-platform npm distribution gate. That enables useful user feedback on the inspection APIs while the first production composition profile is developed with evidence.
