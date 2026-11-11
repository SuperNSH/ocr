//! `TextRecognizer` through the fake backend: resolve-once, concurrency, cancel, timeout, close.

mod common;

use common::{expect_error, image, run, sample_text, settle, spawn_process};
use nsh_ocr::{FakeBackend, RecognitionOptions, TextRecognizer};

#[test]
fn a_call_resolves_once_with_the_recognized_text() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        let call = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.pending()).await;
        assert_eq!(fake.started(), 1);
        assert_eq!(fake.last_image(), Some(image()));
        assert_eq!(fake.last_options(), Some(RecognitionOptions::default()));
        assert!(fake.finish_recognition(Ok(sample_text("Hello, OCR"))));
        let answer = call.await.unwrap().unwrap();
        assert_eq!(answer.text, "Hello, OCR");
        assert!(answer.blocks.is_empty());
        // A second answer has no call left to resolve.
        assert!(!fake.finish_recognition(Ok(sample_text("again"))));
    });
}

#[test]
fn concurrent_calls_each_resolve_once() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        let first = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.pending()).await;
        let second = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.started() == 2).await;
        assert!(fake.pending());
        // Both answers carry the same text, so whichever call is answered first is irrelevant.
        assert!(fake.finish_recognition(Ok(sample_text("both"))));
        assert!(fake.finish_recognition(Ok(sample_text("both"))));
        assert_eq!(first.await.unwrap().unwrap().text, "both");
        assert_eq!(second.await.unwrap().unwrap().text, "both");
        assert!(!fake.finish_recognition(Ok(sample_text("third"))));
        assert_eq!(fake.started(), 2);
    });
}

#[test]
fn cancel_resolves_pending_calls_with_the_cancelled_code() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        let first = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.pending()).await;
        let second = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.started() == 2).await;
        assert!(recognizer.cancel().await);
        assert_eq!(
            expect_error(first.await.unwrap()).code(),
            "ERR_OCR_CANCELLED"
        );
        assert_eq!(
            expect_error(second.await.unwrap()).code(),
            "ERR_OCR_CANCELLED"
        );
        assert!(fake.cancels() >= 1);
    });
}

#[test]
fn cancel_with_no_pending_call_reports_false() {
    let recognizer = TextRecognizer::with_backend(FakeBackend::new());
    run(async {
        assert!(!recognizer.cancel().await);
    });
}

#[test]
fn a_late_answer_after_a_cancel_is_dropped() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        let call = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.pending()).await;
        assert!(recognizer.cancel().await);
        assert_eq!(
            expect_error(call.await.unwrap()).code(),
            "ERR_OCR_CANCELLED"
        );
        assert_eq!(fake.cancels(), 1);
        // The fake still holds the cancelled call's callback; the recognizer drops its answer.
        assert!(fake.finish_recognition(Ok(sample_text("late"))));
        let next = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.started() == 2).await;
        assert!(
            !next.is_finished(),
            "the stale answer must not resolve the new call"
        );
        assert!(fake.finish_recognition(Ok(sample_text("after"))));
        assert_eq!(next.await.unwrap().unwrap().text, "after");
        assert!(!fake.finish_recognition(Ok(sample_text("late again"))));
    });
}

#[test]
fn a_timeout_resolves_with_the_timeout_code_and_the_late_answer_is_dropped() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        // A real short sleep: the fake never answers, so only the deadline can end the call.
        let call = spawn_process(&recognizer, &image(), &options_with_timeout(50));
        settle(|| fake.pending()).await;
        assert_eq!(expect_error(call.await.unwrap()).code(), "ERR_OCR_TIMEOUT");
        // The late answer for the timed-out generation is dropped, so the next call still
        // waits for its own answer and resolves with it.
        assert!(fake.finish_recognition(Ok(sample_text("late"))));
        let next = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.started() == 2).await;
        assert!(
            !next.is_finished(),
            "the late answer must not resolve the new call"
        );
        assert!(fake.finish_recognition(Ok(sample_text("after"))));
        assert_eq!(next.await.unwrap().unwrap().text, "after");
        assert!(!fake.finish_recognition(Ok(sample_text("late again"))));
    });
}

#[test]
fn zero_timeout_is_refused_as_an_invalid_argument() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        let error = expect_error(
            recognizer
                .process_image(&image(), &options_with_timeout(0))
                .await,
        );
        assert_eq!(error.code(), "ERR_OCR_INVALID_ARGUMENT");
        assert_eq!(
            fake.started(),
            0,
            "a zero timeout is refused before any work starts"
        );
    });
}

#[test]
fn dropping_the_call_cancels_the_backend() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        let call = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.pending()).await;
        call.abort();
        let _ = call.await;
        assert_eq!(
            fake.cancels(),
            1,
            "dropping the call asks the backend to cancel"
        );
        // The dropped generation's answer is dropped, and the next call is untouched by it.
        assert!(fake.finish_recognition(Ok(sample_text("late"))));
        let next = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.started() == 2).await;
        assert!(!next.is_finished());
        assert!(fake.finish_recognition(Ok(sample_text("after"))));
        assert_eq!(next.await.unwrap().unwrap().text, "after");
        assert_eq!(fake.cancels(), 1);
    });
}

#[test]
fn a_closed_recognizer_refuses_process_image_with_the_closed_code() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        recognizer.close().await;
        let error = expect_error(
            recognizer
                .process_image(&image(), &RecognitionOptions::default())
                .await,
        );
        assert_eq!(error.code(), "ERR_OCR_CLOSED");
        assert_eq!(fake.started(), 0);
    });
}

#[test]
fn close_is_idempotent() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        let call = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.pending()).await;
        recognizer.close().await;
        recognizer.close().await;
        assert_eq!(
            expect_error(call.await.unwrap()).code(),
            "ERR_OCR_CANCELLED"
        );
        assert_eq!(
            fake.cancels(),
            1,
            "the second close asks the backend nothing new"
        );
        let error = expect_error(
            recognizer
                .process_image(&image(), &RecognitionOptions::default())
                .await,
        );
        assert_eq!(error.code(), "ERR_OCR_CLOSED");
    });
}

#[test]
fn close_resolves_in_flight_calls_as_cancelled() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        let first = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.pending()).await;
        let second = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.started() == 2).await;
        recognizer.close().await;
        assert_eq!(
            expect_error(first.await.unwrap()).code(),
            "ERR_OCR_CANCELLED"
        );
        assert_eq!(
            expect_error(second.await.unwrap()).code(),
            "ERR_OCR_CANCELLED"
        );
        assert!(fake.cancels() >= 1);
        let error = expect_error(
            recognizer
                .process_image(&image(), &RecognitionOptions::default())
                .await,
        );
        assert_eq!(error.code(), "ERR_OCR_CLOSED");
    });
}

/// The options with the opt-in timeout; the struct is non-exhaustive, so fields are set one by one.
#[allow(clippy::field_reassign_with_default)]
fn options_with_timeout(timeout_ms: u64) -> RecognitionOptions {
    let mut options = RecognitionOptions::default();
    options.timeout_ms = Some(timeout_ms);
    options
}
