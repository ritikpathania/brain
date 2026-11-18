# Brain engineering guidelines

## Toolchains and provenance

CI and `rust-toolchain.toml` select Rust 1.99.1. The maintainer host currently maps its `1.99.1-aarch64-apple-darwin` toolchain directory to `stable-aarch64-apple-darwin`, which reports Rust and Cargo 1.100.0. Treat the requested toolchain name and actual command versions as separate facts. Product manifests record the actual assembly commands. Preserve those versions and exact source/artifact identities in verification evidence. Use Bun 1.4.4, TypeScript 5.9.3 and the uv-managed CPython 3.15.2 selected by the frozen daemon project.

## Frontend boundaries

The maintained frontend is `packages/brain-frontend`. Adapt protocol and state under the existing first-party presentation contracts. Frozen visual components, keybindings and oracle fixtures require explicit authorization to change. The retired shell and manual increment PTY scripts remain outside the tracked product tree. See the [Phase 6 acceptance plan](docs/designs/phase-6-legacy-frontend-retirement.md).

## Verification

Run `cargo xtask verify`, frontend typecheck and owned tests, and the real-daemon E2E gate. Run the native archive workflow for the exact release candidate on Linux x86_64 and macOS arm64. Verify the extracted closed product and record source SHA, target, actual tool versions, job URL and archive SHA-256. Preserve unrelated local working files and historical evidence.
