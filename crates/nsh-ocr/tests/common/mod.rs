//! Helpers shared by the integration suites: the runtime, the settle loop, and the fake's inputs.

use std::future::Future;

use nsh_ocr::{ImageInput, OcrError, RecognitionOptions, RecognizedText, TextRecognizer};

/// Runs `future` on a current-thread runtime with a live clock, which the timeout rows need.
pub fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(future)
}

/// Yields until `ready` holds, so a spawned call has reached the backend.
pub async fn settle(ready: impl Fn() -> bool) {
    for _ in 0..1000 {
        if ready() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("the call never reached the backend");
}

/// Any non-empty encoded payload: validation passes, and the fake decodes nothing.
pub fn image() -> ImageInput {
    ImageInput::Encoded(vec![1, 2, 3, 4])
}

/// A minimal successful answer carrying `label` as the joined text.
pub fn sample_text(label: &str) -> RecognizedText {
    RecognizedText {
        text: label.to_owned(),
        blocks: Vec::new(),
    }
}

/// Starts `process_image` on its own task, so the test can answer it while it waits.
pub fn spawn_process(
    recognizer: &TextRecognizer,
    image: &ImageInput,
    options: &RecognitionOptions,
) -> tokio::task::JoinHandle<Result<RecognizedText, OcrError>> {
    let recognizer = recognizer.clone();
    let image = image.clone();
    let options = options.clone();
    tokio::spawn(async move { recognizer.process_image(&image, &options).await })
}

/// Returns a finished call's error, without needing `Debug` on the result tree.
pub fn expect_error<T>(answer: Result<T, OcrError>) -> OcrError {
    match answer {
        Ok(_) => panic!("the call resolved with text, not an error"),
        Err(error) => error,
    }
}
