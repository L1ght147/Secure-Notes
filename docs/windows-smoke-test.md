# Windows portable smoke test

Run this checklist on clean Windows 10 22H2 x64 and Windows 11 x64 virtual
machines without Rust, Visual C++ redistributables, or a prior Secure Notes
installation.

## Prepare the artifact

1. Download the portable ZIP and `SHA256SUMS` from the same workflow run.
2. Verify the ZIP hash.
3. Extract the archive to a writable local folder.
4. Confirm the folder has no settings file before first launch.

## Test the application

1. Launch `SecureNotes.exe` and record any SmartScreen warning.
2. Confirm Russian is selected for a Russian Windows locale and English for a
   non-Russian locale.
3. Switch languages and confirm the visible interface changes immediately.
4. Create a vault with Unicode titles and bodies.
5. Create, rename, search, delete, undo the test by recreating a note, and save
   with **Ctrl+S**.
6. Close with dirty data and exercise **Save**, **Discard**, and **Cancel**.
7. Reopen with the right password, then confirm a wrong password shows the same
   message as a damaged wrapped key.
8. Change the password and confirm the old password no longer works.
9. Trigger the inactivity timeout, Windows lock, and sleep. Confirm each dirty
   session saves before locking.
10. Make the vault folder read-only before a lock. Confirm soft-lock hides the
    notes and offers retry, encrypted copy, and discard.
11. Copy note text. Replace the clipboard from another application before 30
    seconds and confirm Secure Notes preserves it. Repeat without replacement
    and confirm the clipboard clears.
12. Change or delete the vault from another process. Confirm Save refuses to
    overwrite it and offers an encrypted copy.

## Check portable behavior

1. Confirm `secure-notes-settings.json` appears beside the executable.
2. Confirm no Secure Notes directory or file appears in AppData.
3. Search all files created during the test for known note fragments.
4. Restart the VM and reopen the vault.
5. Confirm the executable runs without installing a runtime or service.
