# Secure Notes

Secure Notes is a portable, offline notes application for Windows 10 and 11
x64. It stores every note in one encrypted `.snotes` file. The application has
no network features and doesn't intentionally write note plaintext to disk.

Version 0.1.0 is a prerelease. The application has not received an independent
security audit. Keep backups and complete the Windows smoke-test checklist
before relying on a new build for important data.

## Features

- Create, open, edit, search, rename, and delete notes.
- Encrypt payloads with XChaCha20-Poly1305 and a random 256-bit data key.
- Derive password keys with Argon2id using 256 MiB and three passes.
- Save through a ciphertext-only temporary file and atomic replacement.
- Detect files changed or removed outside Secure Notes.
- Lock after five minutes by default or when Windows locks or sleeps.
- Clear copied text after 30 seconds if no other application replaced it.
- Switch between Russian and English without restarting.

## Use the portable release

1. Download `SecureNotes-windows-x64.zip` and `SHA256SUMS` from the same
   [GitHub release](https://github.com/L1ght147/Secure-Notes/releases).
   Tagged builds are published as prereleases. If no release is listed yet,
   build from source or use a successful workflow artifact.
2. Verify the ZIP SHA-256 hash against `SHA256SUMS`.
3. Extract the ZIP to a writable folder.
4. Run `SecureNotes.exe`.

Windows SmartScreen can warn because the executable is unsigned. Secure Notes
doesn't use an installer, write to AppData, or register system services.
Non-secret settings are stored beside the executable. Vault paths, search text,
and editor state aren't stored in settings.

There is no password recovery. Keep a backup of both the vault and its
password.

## Build from source

On Windows, install Rust 1.92 or later with the MSVC toolchain and the Visual
Studio C++ Build Tools, including the Windows SDK. Then run:

```text
cargo test --all-targets --all-features --locked
cargo build --release --locked --target x86_64-pc-windows-msvc
```

The pinned `libsodium-sys-stable` crate statically embeds libsodium 1.0.22.
The Windows target enables the static C runtime for a portable executable.
eframe persistence is disabled.

CI runs formatting, strict Clippy, and tests on Windows and macOS for pushes
and pull requests. A separate job checks dependencies against RustSec.
Parser fuzzing runs without desktop dependencies on Linux. To build only the
vault and platform library, run `cargo check --no-default-features --lib`.

A tag such as `v0.1.0` triggers Windows packaging and publishes the ZIP and
`SHA256SUMS` to a GitHub prerelease. A manual packaging run only uploads an
Actions artifact. Run the [Windows smoke test](docs/windows-smoke-test.md)
on the packaged executable before marking a release stable.

## Security documentation

- [Security model and limitations](SECURITY.md)
- [Vault format](docs/vault-format.md)
- [Windows smoke-test checklist](docs/windows-smoke-test.md)

This repository doesn't grant a project license. Third-party license notices
are provided separately in `THIRD_PARTY_NOTICES.txt` and in release archives.
