# Local verification

Passed on macOS ARM64, Node.js 24.17.0:

- Release native/TypeScript build.
- Rust formatting and Clippy with warnings denied.
- Public API and example type checks.
- 19 Rust tests, 16 Node API tests and 4 release-artifact integrity tests.
- 3 independent FFmpeg full-decoding/content tests.
- Isolated packed-package installation and CommonJS/ESM/native API checks.

Pending before upload: successful five-platform build and ten exact-tarball consumer jobs, complete artifact manifest, registry upload and matching public integrity.
