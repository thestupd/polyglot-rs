# Publishing an alpha release

The first release is `polyglot-rs@0.1.0-alpha.1`, published under `alpha`. Public metadata names Thieu Quan Ngoc <hi@stupd.dev> and https://github.com/thestupd/polyglot-rs.

The release is one package with five native binaries. No platform child packages, postinstall downloader or consumer Rust compiler are required. Test coverage is macOS 15 ARM64/x64, Ubuntu 24.04 glibc ARM64/x64, and Windows Server 2022 x64, with Node.js 22 and 24. Older operating systems/libc versions and musl are not claimed for this alpha.

## Prepare and verify

1. Bump npm versions with `npm version <version> --workspaces --include-workspace-root --no-git-tag-version --ignore-scripts`; update the changelog. Rust crates are internal/unpublished and maintain their own workspace version.
2. Commit the release source on `release/<version>` and push that branch. The release workflow builds all targets. Each build runs checks before recording binary SHA-256, version, platform and source commit.
3. The assemble job rejects missing, duplicate, changed or mixed-version/commit artifacts. It packs a clean runtime package with no workspace lifecycle scripts. It never rebuilds collected native binaries.
4. Ten consumer jobs install the exact tarball on every target with Node.js 22/24, without a native build. Wait for the complete `Release bundle` workflow to succeed.
5. Download its `npm-release` artifact into `release/`. Keep the local checkout at the source commit recorded in `release/release.json`.

```sh
gh run download <successful-run-id> --repo thestupd/polyglot-rs --name npm-release --dir release
npm run pack:check
npm run test:release -- release/polyglot-rs-0.1.0-alpha.1.tgz
```

If assembling downloaded native artifacts locally instead, run `npm run build:ts --workspace=polyglot-rs` and `npm run release:prepare -- <artifact-directory>`. Only artifacts from a successful workflow qualify for publication. The manifest/hash checks are consistency checks, not a substitute for that successful run.

## Publish

Authenticate to the public registry, then publish the prepared tarball:

```sh
npm login --registry=https://registry.npmjs.org/
npm run release:publish
```

The script checks the release manifest and tarball integrity, verifies the npm identity, checks whether that exact version already exists, and publishes with `--ignore-scripts --tag alpha --access public`. It then compares the registry's SHA-512 integrity against the tested tarball. If a version already exists with matching integrity, rerunning is safe; a different existing artifact fails without overwriting it. A registry version is immutable. npm may require interactive authentication/2FA.

Publication is a separate explicit action; build/prepare/test commands never upload anything. No Git tag, GitHub release, PR merge or `latest` tag promotion is performed by this npm release path. The initial empty repository can publish source from its bootstrap release branch without committing directly to `main`.

## Verify and recover

Check `npm view polyglot-rs@0.1.0-alpha.1 version dist.integrity --registry=https://registry.npmjs.org/` and `npm view polyglot-rs dist-tags --registry=https://registry.npmjs.org/`. Install that exact version into a clean consumer and exercise both CommonJS and ESM.

If upload status is uncertain, query the exact version and compare integrity before retrying. Never replace a published version or silently promote an alpha to `latest`. A future revision needs a new version and a new complete build/consumer run.
