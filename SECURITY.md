# Security model

Secure Notes protects a closed vault and reduces plaintext traces during an
open session. It doesn't claim to defend against an administrator, malware,
kernel dumps, hibernation images, or physical RAM extraction.

## Protected data

- Note titles and bodies are encrypted inside the vault.
- A random data-encryption key encrypts the JSON payload.
- Argon2id derives a key-encryption key from the password.
- XChaCha20-Poly1305 authenticates the wrapped key, header, and payload.
- Data keys use guarded allocations, page locking, and explicit zeroing.
- Password fields are explicitly cleared after each operation.
- Save operations write ciphertext to a temporary file in the vault folder.

## Operational limits

- The decrypted JSON payload must not exceed 100 MiB.
- Passwords must contain at least eight Unicode characters.
- Password recovery, export, synchronization, attachments, and plugins aren't
  available.
- Only one vault can be open at a time. Don't open the same vault in multiple
  Secure Notes processes.
- The application is unsigned, so Windows SmartScreen can display a warning.

## Lock behavior

Secure Notes saves dirty data before an inactivity or Windows-session lock. If
the save fails, the interface enters soft-lock: it hides all notes but retains
the session and key in memory. Enter the password to retry the save, save an
encrypted copy, or discard the in-memory changes. A successful retry returns to
the fully locked state and releases the session key.

Copied text is scheduled for clearing after 30 seconds. Secure Notes compares
the current clipboard fingerprint before clearing, so it doesn't erase content
that another application placed there.

## Files written by the application

- The selected `.snotes` vault.
- Ciphertext-only temporary files beside the vault during a save.
- `secure-notes-settings.json` beside `SecureNotes.exe`.

The settings file contains only language, inactivity timeout, and the Windows
lock/sleep preference. It doesn't contain vault paths, file history, search
text, passwords, note text, or editor state.

## Report a vulnerability

No public security contact is configured yet. Don't publish sensitive exploit
details in a public issue. Repository owners must add a private reporting
channel before accepting external vulnerability reports.
