# Vault format version 1

All integer fields use little-endian encoding. The fixed header is 168 bytes.
The complete header is authenticated as associated data for the payload.

| Offset | Size | Field |
| ---: | ---: | --- |
| 0 | 8 | Magic: `SNOTES\0\0` |
| 8 | 2 | Container version: `1` |
| 10 | 2 | Reserved: zero |
| 12 | 16 | Random vault ID |
| 28 | 16 | Argon2id salt |
| 44 | 8 | Argon2id operation limit: `3` |
| 52 | 8 | Argon2id memory limit: `268435456` bytes |
| 60 | 4 | libsodium algorithm ID: Argon2id 1.3 |
| 64 | 24 | Wrapped-key XChaCha20-Poly1305 nonce |
| 88 | 48 | Encrypted 32-byte data key and 16-byte tag |
| 136 | 24 | Payload XChaCha20-Poly1305 nonce |
| 160 | 8 | Encrypted payload length |
| 168 | variable | Encrypted JSON payload and 16-byte tag |

The wrapped-key associated data is header bytes 0 through 87. The payload
associated data is the complete fixed header. The parser validates the file
length and the 100 MiB plaintext limit before allocating payload output.

The decrypted JSON object has schema version 1 and a `notes` array. Each note
contains a UUID, title, body, and Unix modification timestamp.
