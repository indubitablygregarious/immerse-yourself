//! Shared audio output singleton.
//!
//! `OutputStream` is `!Send + !Sync` (it wraps a cpal Stream with a raw pointer),
//! so it must live on the thread that created it. We spawn a dedicated thread to
//! hold the OutputStream alive and share only the `OutputStreamHandle` (which is
//! `Send + Sync`) via a `OnceLock`.

use std::sync::OnceLock;

use rodio::OutputStreamHandle;

static AUDIO_HANDLE: OnceLock<Option<OutputStreamHandle>> = OnceLock::new();

/// Returns a reference to the shared `OutputStreamHandle`, or `None` if no
/// audio device is available.
pub fn get_output_stream_handle() -> Option<&'static OutputStreamHandle> {
    AUDIO_HANDLE
        .get_or_init(|| {
            let (tx, rx) = std::sync::mpsc::sync_channel(1);

            std::thread::Builder::new()
                .name("audio-output".into())
                .spawn(move || {
                    match rodio::OutputStream::try_default() {
                        Ok((_stream, handle)) => {
                            let _ = tx.send(Some(handle));
                            // Park forever to keep _stream alive for the process lifetime.
                            loop {
                                std::thread::park();
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to initialize audio output: {}", e);
                            let _ = tx.send(None);
                        }
                    }
                })
                .ok();

            rx.recv().unwrap_or(None)
        })
        .as_ref()
}
