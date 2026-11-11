//! A backend with scripted answers. Tests use it, and so can an app that runs without an engine.

use std::{
    collections::VecDeque,
    future::ready,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use crate::{
    backend::{Backend, BackendFuture, RecognitionDone},
    error::OcrError,
    input::ImageInput,
    options::RecognitionOptions,
    text::RecognizedText,
};

/// A backend whose answers the test sets, and whose recognitions the test finishes.
///
/// Like the platform engines, a cancel does not finish a recognition: the test answers it, as
/// the engine would. Several recognitions can wait at once, and each `finish_recognition`
/// answers one of them. Clones share one state.
#[derive(Clone)]
pub struct FakeBackend {
    state: Arc<Mutex<FakeState>>,
}

struct FakeState {
    supported: Result<Vec<String>, OcrError>,
    next_start_error: Option<OcrError>,
    waiting: VecDeque<RecognitionDone>,
    started: usize,
    cancels: usize,
    last_image: Option<ImageInput>,
    last_options: Option<RecognitionOptions>,
}

impl Default for FakeBackend {
    /// An engine that reads English, with nothing waiting and nothing scripted to fail.
    fn default() -> Self {
        Self::new()
    }
}

impl FakeBackend {
    /// An engine that reads English, with nothing waiting and nothing scripted to fail.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                supported: Ok(vec!["en-US".to_owned()]),
                next_start_error: None,
                waiting: VecDeque::new(),
                started: 0,
                cancels: 0,
                last_image: None,
                last_options: None,
            })),
        }
    }

    fn lock(&self) -> MutexGuard<'_, FakeState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sets what `supported_languages` answers.
    pub fn set_supported_languages(&self, answer: Result<Vec<String>, OcrError>) {
        self.lock().supported = answer;
    }

    /// Makes the next recognition fail to start with `error`, as a platform does when it cannot
    /// run one.
    pub fn fail_next_recognition(&self, error: OcrError) {
        self.lock().next_start_error = Some(error);
    }

    /// Whether any recognition waits for its answer.
    pub fn pending(&self) -> bool {
        !self.lock().waiting.is_empty()
    }

    /// Answers one waiting recognition. Returns `false` when none waits.
    pub fn finish_recognition(&self, answer: Result<RecognizedText, OcrError>) -> bool {
        let waiting = self.lock().waiting.pop_front();
        match waiting {
            Some(done) => {
                done(answer);
                true
            }
            None => false,
        }
    }

    /// How many recognitions were started.
    pub fn started(&self) -> usize {
        self.lock().started
    }

    /// How many times the backend was asked to cancel.
    pub fn cancels(&self) -> usize {
        self.lock().cancels
    }

    /// The image the last started recognition was given.
    pub fn last_image(&self) -> Option<ImageInput> {
        self.lock().last_image.clone()
    }

    /// The options the last started recognition was given.
    pub fn last_options(&self) -> Option<RecognitionOptions> {
        self.lock().last_options.clone()
    }
}

impl Backend for FakeBackend {
    fn supported_languages(&self) -> BackendFuture<'_, Result<Vec<String>, OcrError>> {
        Box::pin(ready(self.lock().supported.clone()))
    }

    fn start_recognition(
        &self,
        image: &ImageInput,
        options: &RecognitionOptions,
        done: RecognitionDone,
    ) -> Result<(), OcrError> {
        let mut state = self.lock();
        state.last_image = Some(image.clone());
        state.last_options = Some(options.clone());
        if let Some(error) = state.next_start_error.take() {
            return Err(error);
        }
        state.started += 1;
        state.waiting.push_back(done);
        Ok(())
    }

    fn cancel_recognition(&self) {
        self.lock().cancels += 1;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::error::ErrorKind;
    use crate::text::RecognizedText;

    use super::*;

    /// A done-callback plus the flag recording whether it ever fired.
    fn done_recorder() -> (RecognitionDone, Arc<AtomicBool>) {
        let fired = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&fired);
        (Box::new(move |_| flag.store(true, Ordering::SeqCst)), fired)
    }

    #[test]
    fn a_failed_start_records_the_call_but_counts_nothing_started() {
        let fake = FakeBackend::new();
        fake.fail_next_recognition(OcrError::new(ErrorKind::ModelNotLoaded));
        let (done, fired) = done_recorder();
        let error = fake
            .start_recognition(
                &ImageInput::Encoded(vec![1]),
                &RecognitionOptions::default(),
                done,
            )
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ModelNotLoaded);
        assert_eq!(fake.started(), 0);
        assert!(!fake.pending());
        assert_eq!(fake.last_image(), Some(ImageInput::Encoded(vec![1])));
        assert!(!fired.load(Ordering::SeqCst));
    }

    #[test]
    fn waiting_recognitions_are_answered_one_per_finish() {
        let fake = FakeBackend::new();
        let sample = || Ok(RecognizedText::default());
        let (first, _) = done_recorder();
        let (second, _) = done_recorder();
        assert_eq!(
            fake.start_recognition(
                &ImageInput::Encoded(vec![1]),
                &RecognitionOptions::default(),
                first
            ),
            Ok(())
        );
        assert_eq!(
            fake.start_recognition(
                &ImageInput::Encoded(vec![2]),
                &RecognitionOptions::default(),
                second
            ),
            Ok(())
        );
        assert_eq!(fake.started(), 2);
        assert!(fake.pending());
        assert!(fake.finish_recognition(sample()));
        assert!(fake.pending());
        assert!(fake.finish_recognition(sample()));
        assert!(!fake.pending());
        assert!(!fake.finish_recognition(sample()));
        assert_eq!(fake.cancels(), 0);
    }

    #[test]
    fn supported_languages_answer_the_scripted_value() {
        let fake = FakeBackend::new();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert_eq!(
            runtime.block_on(fake.supported_languages()).unwrap(),
            vec!["en-US".to_owned()]
        );
        fake.set_supported_languages(Err(OcrError::new(ErrorKind::NotAvailable)));
        assert_eq!(
            runtime
                .block_on(fake.supported_languages())
                .unwrap_err()
                .kind(),
            ErrorKind::NotAvailable
        );
    }
}
