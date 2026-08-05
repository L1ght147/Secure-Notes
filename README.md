# Secure Notes

Portable offline encrypted notes for Windows 10/11 x64.

The application stores all notes in a single `.snotes` vault. No plaintext note
content is intentionally written to disk, and no network feature is present.

## Development

The project uses Rust 2024 and eframe/egui. Persistence support in eframe is
disabled; application settings are limited to non-secret preferences stored next
to the executable.
