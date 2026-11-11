//! `TextRecognizer`: the calls an app makes, with concurrent recognitions and one answer per call.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError},
};

#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use tokio::sync::oneshot;

use crate::{
    backend::{Backend, RecognitionDone},
    error::{ErrorKind, OcrError},
    input::ImageInput,
    options::RecognitionOptions,
    text::RecognizedText,
};

type Answer = Result<RecognizedText, OcrError>;

/// The pending calls, each under its generation id. A generation that is no longer registered
/// has resolved - by its answer, a cancel, a close, a timeout or a dropped future - and a late
/// backend answer for it is dropped, so no call resolves twice.
#[derive(Default)]
struct Pending {
    next_id: u64,
    calls: HashMap<u64, oneshot::Sender<Answer>>,
    closed: bool,
}

struct Shared {
    backend: Arc<dyn Backend>,
    pending: Mutex<Pending>,
}

impl Shared {
    fn pending(&self) -> MutexGuard<'_, Pending> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Text recognition for an app, as Flutter's text recognizers shape it.
///
/// Clones share one backend and one registry of pending calls. Calls run concurrently
/// (recognition is compute, not a prompt); each resolves exactly once.
#[derive(Clone)]
pub struct TextRecognizer {
    shared: Arc<Shared>,
}

impl TextRecognizer {
    /// The platform's recognizer, one shared handle for the whole process.
    pub fn new() -> Self {
        static PLATFORM: OnceLock<TextRecognizer> = OnceLock::new();
        PLATFORM
            .get_or_init(|| Self::from_arc(crate::platform::default_backend()))
            .clone()
    }

    /// Recognition through `backend`, with its own registry. Tests and fakes use this.
    pub fn with_backend(backend: impl Backend + 'static) -> Self {
        Self::from_arc(Arc::new(backend))
    }

    fn from_arc(backend: Arc<dyn Backend>) -> Self {
        Self {
            shared: Arc::new(Shared {
                backend,
                pending: Mutex::new(Pending::default()),
            }),
        }
    }

    /// The BCP-47 languages the backend can recognise now.
    pub async fn supported_languages(&self) -> Result<Vec<String>, OcrError> {
        self.shared.backend.supported_languages().await
    }

    /// Recognises text in `image` and waits for the answer.
    ///
    /// An image with no text resolves `Ok` with empty text and no blocks. Every other outcome
    /// is an error with a code: an invalid image, a refused option, a closed recognizer, a
    /// cancel, a timeout. Dropping the future asks the backend to stop and deregisters the
    /// call, whose late answer is then dropped. With `timeout_ms` set, the call gives up at the
    /// deadline with [`ErrorKind::Timeout`]; the answer that arrives after it is dropped too.
    pub async fn process_image(
        &self,
        image: &ImageInput,
        options: &RecognitionOptions,
    ) -> Result<RecognizedText, OcrError> {
        image.validate()?;
        if options.timeout_ms == Some(0) {
            return Err(OcrError::new(ErrorKind::InvalidArgument));
        }
        let (sender, receiver) = oneshot::channel();
        let generation = {
            let mut pending = self.shared.pending();
            if pending.closed {
                return Err(OcrError::new(ErrorKind::Closed));
            }
            let generation = pending.next_id;
            pending.next_id += 1;
            pending.calls.insert(generation, sender);
            generation
        };
        let shared = Arc::clone(&self.shared);
        let done: RecognitionDone = Box::new(move |answer| answered(&shared, generation, answer));
        if let Err(error) = self.shared.backend.start_recognition(image, options, done) {
            self.shared.pending().calls.remove(&generation);
            return Err(error);
        }
        let mut owner = Owner {
            shared: Arc::clone(&self.shared),
            generation,
            armed: true,
        };
        // The timer needs tokio's time driver, which wasm32 has no runtime for; there the
        // option is accepted and unused, and the TypeScript layer runs its own watchdog.
        #[cfg(not(target_arch = "wasm32"))]
        let answer = match options.timeout_ms {
            Some(timeout_ms) => {
                tokio::select! {
                    answer = receiver => answer,
                    _ = tokio::time::sleep(Duration::from_millis(timeout_ms)) => {
                        owner.armed = false;
                        deregister(&self.shared, generation);
                        Ok(Err(OcrError::new(ErrorKind::Timeout)))
                    }
                }
            }
            None => receiver.await,
        };
        #[cfg(target_arch = "wasm32")]
        let answer = receiver.await;
        owner.armed = false;
        answer.unwrap_or_else(|_| Err(OcrError::new(ErrorKind::Backend)))
    }

    /// Cancels every pending call. Each resolves with [`ErrorKind::Cancelled`]; the answers the
    /// backend still delivers for them are dropped. Returns `true` when any call was pending.
    pub async fn cancel(&self) -> bool {
        self.resolve_pending(ErrorKind::Cancelled)
    }

    /// Closes the recognizer. The first close resolves the in-flight calls with
    /// [`ErrorKind::Cancelled`] and marks it closed; every later call refuses with
    /// [`ErrorKind::Closed`]. Further closes do nothing.
    pub async fn close(&self) {
        let first = {
            let mut pending = self.shared.pending();
            if pending.closed {
                false
            } else {
                pending.closed = true;
                true
            }
        };
        if first {
            self.resolve_pending(ErrorKind::Cancelled);
        }
    }

    /// Resolves every pending call with `kind` and asks the backend to stop, once, when any
    /// call was pending.
    fn resolve_pending(&self, kind: ErrorKind) -> bool {
        let senders = {
            let mut pending = self.shared.pending();
            pending
                .calls
                .drain()
                .map(|(_, sender)| sender)
                .collect::<Vec<_>>()
        };
        if senders.is_empty() {
            return false;
        }
        for sender in senders {
            let _ = sender.send(Err(OcrError::new(kind)));
        }
        self.shared.backend.cancel_recognition();
        true
    }
}

impl Default for TextRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Hands the backend's answer to its waiting call. An answer for a generation that is no longer
/// registered - the call timed out, was cancelled, or was dropped - is dropped with it.
fn answered(shared: &Shared, generation: u64, answer: Answer) {
    let sender = shared.pending().calls.remove(&generation);
    if let Some(sender) = sender {
        let _ = sender.send(answer);
    }
}

/// Deregisters a timed-out call, so its late answer finds nothing to resolve.
#[cfg(not(target_arch = "wasm32"))]
fn deregister(shared: &Shared, generation: u64) {
    shared.pending().calls.remove(&generation);
}

/// Cancels the recognition if the call's future is dropped before the answer arrives.
struct Owner {
    shared: Arc<Shared>,
    generation: u64,
    armed: bool,
}

impl Drop for Owner {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // Only the owner's own generation: after a timeout or a cancel freed it, a newer
        // call's sender must survive the stale owner's drop.
        if self
            .shared
            .pending()
            .calls
            .remove(&self.generation)
            .is_some()
        {
            self.shared.backend.cancel_recognition();
        }
    }
}
