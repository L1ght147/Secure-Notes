use super::{ClipboardBackend, SessionEvent, clear_clipboard_if_unchanged, decode_session_event};
use std::sync::Mutex;

struct FakeClipboard {
    value: Mutex<Option<String>>,
}

impl FakeClipboard {
    fn new(value: &str) -> Self {
        Self {
            value: Mutex::new(Some(value.into())),
        }
    }

    fn value(&self) -> Option<String> {
        self.value.lock().unwrap().clone()
    }
}

impl ClipboardBackend for FakeClipboard {
    fn text(&self) -> Option<String> {
        self.value()
    }

    fn clear(&self) -> Result<(), ()> {
        *self.value.lock().unwrap() = Some(String::new());
        Ok(())
    }
}

#[test]
fn clipboard_is_cleared_only_while_original_value_remains() {
    let original = FakeClipboard::new("secret note");
    let marker = blake3::hash(b"secret note");
    clear_clipboard_if_unchanged(&original, marker).unwrap();
    assert_eq!(original.value().as_deref(), Some(""));

    let replaced = FakeClipboard::new("changed elsewhere");
    clear_clipboard_if_unchanged(&replaced, marker).unwrap();
    assert_eq!(replaced.value().as_deref(), Some("changed elsewhere"));
}

#[test]
fn session_events_distinguish_lock_sleep_and_resume() {
    assert_eq!(
        decode_session_event(0x02B1, 0x7),
        Some(SessionEvent::Locked)
    );
    assert_eq!(
        decode_session_event(0x0218, 0x0004),
        Some(SessionEvent::Sleeping)
    );
    assert_eq!(
        decode_session_event(0x0218, 0x0012),
        Some(SessionEvent::Resumed)
    );
    assert_eq!(decode_session_event(0x9999, 0), None);
}
