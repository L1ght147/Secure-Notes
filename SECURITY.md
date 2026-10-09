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
- Current note allocations are explicitly zeroed when notes are dropped.
- Password fields are cleared after attempted operations and cancelled dialogs.
- Locking the vault clears secret fields, search text, and editor undo state.
- Save operations write ciphertext to a temporary file in the vault folder.

Note strings, password input, and UI rendering use ordinary heap memory.
Clearing an editor's undo state releases its cached copies; it does not
securely zero every allocation made by egui. Reallocation, rendering caches,
OS input, and clipboard services can leave additional plaintext copies.
The application does not guarantee that all plaintext traces are erased.

## Operational limits

- The decrypted JSON payload must not exceed 100 MiB.
- Passwords must contain at least eight Unicode characters.
- Password recovery, export, synchronization, attachments, and plugins aren't
  available.
- Only one vault can be open in each process. Concurrent Secure Notes writers
  use an OS lock on a persistent, empty `.<vault-name>.lock` sidecar. A stale
  session cannot overwrite a successful save from another process.
- External changes are checked before encryption and again before replacement.
  Uncooperative external writers can still race the final check. Don't edit or
  replace an open vault from another application, and keep it on a local disk.
  Don't delete the lock sidecar while any process is using the vault.
- The application is unsigned, so Windows SmartScreen can display a warning.

## Lock behavior

Secure Notes saves dirty data before an inactivity or Windows-session lock. If
the save fails, the interface enters soft-lock: it hides all notes but retains
the session and key in memory. Enter the password to retry the save, save an
encrypted copy, or discard the in-memory changes. Saving a copy from soft-lock
requires the original password and a new password of at least eight characters.
An encrypted key envelope retained in memory permits this check even when the
original file has been deleted. A successful retry returns to
the fully locked state and releases the session key.

If Windows session monitoring cannot start or later disconnects, the interface
displays a warning. The inactivity timer still works, but you must lock the
vault manually before locking Windows or putting the computer to sleep.

Copied text is scheduled for clearing after 30 seconds while the application
is running. Exiting before the timer fires prevents automatic clearing; clear
the clipboard yourself in that case. Secure Notes compares
the current clipboard fingerprint before clearing, so it doesn't erase content
that another application placed there.

## Files written by the application

- The selected `.snotes` vault.
- Ciphertext-only temporary files beside the vault during a save.
- Empty, persistent `.<vault-name>.lock` sidecars used to coordinate writers.
- `secure-notes-settings.json` beside `SecureNotes.exe`.

The settings file contains only language, inactivity timeout, the Windows
lock/sleep preference, theme, and note sort order. It doesn't contain vault
paths, file history, search text, passwords, note text, or editor state.

## Report a vulnerability

Email [14012006ii@gmail.com](mailto:14012006ii@gmail.com) privately with the
version, affected platform, reproduction steps, and expected impact. Don't
publish passwords, real vault contents, or exploit details in public issues.
Use a synthetic vault when sharing a reproduction. No response-time guarantee
or independent security certification is provided.
