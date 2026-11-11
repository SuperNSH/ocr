//! The backend of a target with no recognizer: nothing is available, nothing starts.

use std::future::ready;

use crate::{
    backend::{Backend, BackendFuture, RecognitionDone},
    error::{ErrorKind, OcrError},
    input::ImageInput,
    options::RecognitionOptions,
};

/// A backend that recognises nothing and starts nothing.
pub(crate) struct NativeBackend;

impl Backend for NativeBackend {
    fn supported_languages(&self) -> BackendFuture<'_, Result<Vec<String>, OcrError>> {
        Box::pin(ready(Ok(Vec::new())))
    }

    fn start_recognition(
        &self,
        _image: &ImageInput,
        _options: &RecognitionOptions,
        _done: RecognitionDone,
    ) -> Result<(), OcrError> {
        Err(OcrError::new(ErrorKind::NotAvailable))
    }

    fn cancel_recognition(&self) {
        // Nothing ever starts, so there is nothing to stop.
    }
}
