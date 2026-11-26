# Local verification

Passed on macOS ARM64, Node.js 24.17.0:

- Release native/TypeScript build.
- Rust formatting and Clippy with warnings denied.
- Public API and example type checks.
- 19 Rust tests, 16 Node API tests and 4 release-artifact integrity tests.
- 3 independent FFmpeg full-decoding/content tests.
- Isolated packed-package installation and CommonJS/ESM/native API checks.

Pending before upload: successful five-platform build and ten exact-tarball consumer jobs, complete artifact manifest, registry upload and matching public integrity.

## Release verification (2026-11-26)

- Release bundle run 45827832214 on 4daccba: 5 platform builds, assemble and 10 exact-tarball install jobs (Node 22/24) passed.
- `release.cjs check` passed on the downloaded tarball (sha256 58d50cc1…).
- Published `polyglot-rs@0.1.0-alpha.1` to registry.npmjs.org; dist-tags `alpha` and `latest` → 0.1.0-alpha.1.
- Registry dist.integrity matches the tested tarball; fresh install loads via CommonJS and ESM on macOS ARM64.
