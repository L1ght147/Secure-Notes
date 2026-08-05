//! Parser entry points used only by the cargo-fuzz harnesses.

use super::{Header, KdfProfile, PAYLOAD_LIMIT, Vault};

pub fn parse_container(input: &[u8]) {
    let _ = Header::parse(input, KdfProfile::production());
}

pub fn parse_payload(input: &[u8]) {
    if input.len() <= PAYLOAD_LIMIT {
        let _ = serde_json::from_slice::<Vault>(input);
    }
}
