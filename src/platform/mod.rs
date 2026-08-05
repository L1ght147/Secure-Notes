//! Operating-system security integration.

use std::{sync::mpsc::Receiver, time::Duration};

use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionEvent {
    Locked,
    Sleeping,
    Resumed,
}

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("secure memory could not be locked")]
    MemoryLockFailed,
    #[error("session event monitoring is unavailable")]
    SessionEventsUnavailable,
    #[error("clipboard is unavailable")]
    ClipboardUnavailable,
}

pub trait PlatformSecurity {
    fn lock_memory(&self, bytes: &mut [u8]) -> Result<(), PlatformError>;
    fn unlock_memory(&self, bytes: &mut [u8]);
    fn subscribe_session_events(&self) -> Result<Receiver<SessionEvent>, PlatformError>;
    fn set_clipboard_with_expiry(&self, text: &str, expiry: Duration) -> Result<(), PlatformError>;
}

#[derive(Clone, Copy, Default)]
pub struct NativePlatformSecurity;

impl PlatformSecurity for NativePlatformSecurity {
    fn lock_memory(&self, bytes: &mut [u8]) -> Result<(), PlatformError> {
        lock_memory(bytes)
    }

    fn unlock_memory(&self, bytes: &mut [u8]) {
        unlock_memory(bytes);
    }

    fn subscribe_session_events(&self) -> Result<Receiver<SessionEvent>, PlatformError> {
        subscribe_session_events()
    }

    fn set_clipboard_with_expiry(&self, text: &str, expiry: Duration) -> Result<(), PlatformError> {
        let mut clipboard =
            arboard::Clipboard::new().map_err(|_| PlatformError::ClipboardUnavailable)?;
        clipboard
            .set_text(text)
            .map_err(|_| PlatformError::ClipboardUnavailable)?;

        let marker = blake3::hash(text.as_bytes());
        std::thread::Builder::new()
            .name("secure-notes-clipboard-expiry".into())
            .spawn(move || {
                std::thread::sleep(expiry);
                let _ = clear_clipboard_if_unchanged(&SystemClipboard, marker);
            })
            .map_err(|_| PlatformError::ClipboardUnavailable)?;
        Ok(())
    }
}

trait ClipboardBackend: Send + Sync {
    fn text(&self) -> Option<String>;
    fn clear(&self) -> Result<(), ()>;
}

struct SystemClipboard;

impl ClipboardBackend for SystemClipboard {
    fn text(&self) -> Option<String> {
        arboard::Clipboard::new().ok()?.get_text().ok()
    }

    fn clear(&self) -> Result<(), ()> {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(String::new()))
            .map_err(|_| ())
    }
}

fn clear_clipboard_if_unchanged(
    clipboard: &dyn ClipboardBackend,
    expected: blake3::Hash,
) -> Result<(), ()> {
    let Some(current) = clipboard.text() else {
        return Ok(());
    };
    if blake3::hash(current.as_bytes()) == expected {
        clipboard.clear()?;
    }
    Ok(())
}

#[cfg(any(windows, test))]
fn decode_session_event(message: u32, parameter: u32) -> Option<SessionEvent> {
    match (message, parameter) {
        (0x02B1, 0x7) => Some(SessionEvent::Locked),
        (0x0218, 0x0004) => Some(SessionEvent::Sleeping),
        (0x0218, 0x0006 | 0x0007 | 0x0008 | 0x0012) => Some(SessionEvent::Resumed),
        _ => None,
    }
}

#[cfg(windows)]
fn lock_memory(bytes: &mut [u8]) -> Result<(), PlatformError> {
    unsafe { windows::Win32::System::Memory::VirtualLock(bytes.as_ptr().cast(), bytes.len()) }
        .map_err(|_| PlatformError::MemoryLockFailed)
}

#[cfg(not(windows))]
fn lock_memory(bytes: &mut [u8]) -> Result<(), PlatformError> {
    let result = unsafe { libsodium_sys::sodium_mlock(bytes.as_mut_ptr().cast(), bytes.len()) };
    if result == 0 {
        Ok(())
    } else {
        Err(PlatformError::MemoryLockFailed)
    }
}

#[cfg(windows)]
fn unlock_memory(bytes: &mut [u8]) {
    let _ = unsafe {
        windows::Win32::System::Memory::VirtualUnlock(bytes.as_ptr().cast(), bytes.len())
    };
}

#[cfg(not(windows))]
fn unlock_memory(bytes: &mut [u8]) {
    unsafe {
        let _ = libsodium_sys::sodium_munlock(bytes.as_mut_ptr().cast(), bytes.len());
    }
}

#[cfg(windows)]
fn subscribe_session_events() -> Result<Receiver<SessionEvent>, PlatformError> {
    windows_events::subscribe()
}

#[cfg(not(windows))]
fn subscribe_session_events() -> Result<Receiver<SessionEvent>, PlatformError> {
    let (_sender, receiver) = std::sync::mpsc::channel();
    Ok(receiver)
}

#[cfg(windows)]
mod windows_events {
    use super::{PlatformError, SessionEvent, decode_session_event};
    use std::sync::{OnceLock, mpsc};
    use windows::{
        Win32::{
            Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
            System::{
                LibraryLoader::GetModuleHandleW,
                RemoteDesktop::{NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification},
            },
            UI::WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG,
                RegisterClassW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW,
            },
        },
        core::w,
    };

    static EVENT_SENDER: OnceLock<mpsc::Sender<SessionEvent>> = OnceLock::new();

    pub(super) fn subscribe() -> Result<mpsc::Receiver<SessionEvent>, PlatformError> {
        let (sender, receiver) = mpsc::channel();
        EVENT_SENDER
            .set(sender)
            .map_err(|_| PlatformError::SessionEventsUnavailable)?;
        std::thread::Builder::new()
            .name("secure-notes-session-events".into())
            .spawn(message_loop)
            .map_err(|_| PlatformError::SessionEventsUnavailable)?;
        Ok(receiver)
    }

    fn message_loop() {
        unsafe {
            let Ok(module) = GetModuleHandleW(None) else {
                return;
            };
            let instance = HINSTANCE(module.0);
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: w!("SecureNotesSessionEvents"),
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 {
                return;
            }
            let Ok(window) = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("SecureNotesSessionEvents"),
                w!(""),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance),
                None,
            ) else {
                return;
            };
            if WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION).is_err() {
                return;
            }
            let mut message = MSG::default();
            while GetMessageW(&mut message, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    unsafe extern "system" fn window_proc(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if let Some(event) = decode_session_event(message, wparam.0 as u32)
            && let Some(sender) = EVENT_SENDER.get()
        {
            let _ = sender.send(event);
        }
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    }
}

#[cfg(test)]
mod tests;
