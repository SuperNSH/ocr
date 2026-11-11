//! The seam between `TextRecognizer` and a platform: the language query, and one recognition
//! that answers once.

use std::{future::Future, pin::Pin};

use crate::{
    error::OcrError, input::ImageInput, options::RecognitionOptions, text::RecognizedText,
};

/// A future a backend returns. Each call's future borrows the backend.
pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// Receives the backend's answer to one recognition, exactly once, from any thread.
pub type RecognitionDone = Box<dyn FnOnce(Result<RecognizedText, OcrError>) + Send>;

/// What a platform provides. [`crate::TextRecognizer`] adds the rules: concurrent calls, one
/// answer per call.
pub trait Backend: Send + Sync {
    /// The BCP-47 languages this backend can recognise now.
    fn supported_languages(&self) -> BackendFuture<'_, Result<Vec<String>, OcrError>>;

    /// Starts recognition and returns at once. `done` is called exactly once, from any thread,
    /// when the answer exists. An `Err` return means nothing was started and `done` is never
    /// called.
    fn start_recognition(
        &self,
        image: &ImageInput,
        options: &RecognitionOptions,
        done: RecognitionDone,
    ) -> Result<(), OcrError>;

    /// Asks the backend to stop. Where the platform cannot stop running work this does nothing,
    /// and the answer still arrives through `done` - the core drops it if the call already
    /// resolved.
    fn cancel_recognition(&self);
}
