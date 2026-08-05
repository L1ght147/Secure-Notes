#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    secure_notes::vault::fuzzing::parse_container(data);

    // Preserve a valid fixed header so mutations reach every length check
    // instead of almost always stopping at the magic bytes.
    let mut structured = vec![0_u8; 168 + data.len()];
    structured[..8].copy_from_slice(b"SNOTES\0\0");
    structured[8..10].copy_from_slice(&1_u16.to_le_bytes());
    structured[44..52].copy_from_slice(&3_u64.to_le_bytes());
    structured[52..60].copy_from_slice(&(256_u64 * 1024 * 1024).to_le_bytes());
    structured[60..64].copy_from_slice(&2_u32.to_le_bytes());
    structured[160..168].copy_from_slice(&(data.len() as u64).to_le_bytes());
    structured[168..].copy_from_slice(data);
    secure_notes::vault::fuzzing::parse_container(&structured);
});
