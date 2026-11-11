//! Validation refusals and the error kinds, each reaching the caller with its frozen code.

mod common;

use common::{expect_error, image, run, sample_text, settle, spawn_process};
use nsh_ocr::{
    ErrorKind, FakeBackend, ImageInput, OcrError, RecognitionOptions, Script, TextRecognizer,
};

const ALL_KINDS: [(ErrorKind, &str); 10] = [
    (ErrorKind::NotAvailable, "ERR_OCR_NOT_AVAILABLE"),
    (
        ErrorKind::UnsupportedLanguage,
        "ERR_OCR_UNSUPPORTED_LANGUAGE",
    ),
    (ErrorKind::ModelNotLoaded, "ERR_OCR_MODEL_NOT_LOADED"),
    (ErrorKind::InvalidImage, "ERR_OCR_INVALID_IMAGE"),
    (ErrorKind::InvalidArgument, "ERR_OCR_INVALID_ARGUMENT"),
    (ErrorKind::IoFailed, "ERR_OCR_IO_FAILED"),
    (ErrorKind::Closed, "ERR_OCR_CLOSED"),
    (ErrorKind::Cancelled, "ERR_OCR_CANCELLED"),
    (ErrorKind::Timeout, "ERR_OCR_TIMEOUT"),
    (ErrorKind::Backend, "ERR_OCR_BACKEND"),
];

#[test]
fn an_rgba_size_mismatch_is_refused_as_an_invalid_image() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        // 2x2 RGBA needs 16 bytes; 15 is a torn read.
        let torn = ImageInput::Rgba {
            width: 2,
            height: 2,
            data: vec![0; 15],
        };
        assert_eq!(torn.validate().unwrap_err().code(), "ERR_OCR_INVALID_IMAGE");
        let error = expect_error(
            recognizer
                .process_image(&torn, &RecognitionOptions::default())
                .await,
        );
        assert_eq!(error.code(), "ERR_OCR_INVALID_IMAGE");
        assert_eq!(fake.started(), 0);
    });
}

#[test]
fn zero_dimensions_are_refused_as_an_invalid_image() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        for torn in [
            ImageInput::Rgba {
                width: 0,
                height: 2,
                data: Vec::new(),
            },
            ImageInput::Rgba {
                width: 2,
                height: 0,
                data: Vec::new(),
            },
        ] {
            assert_eq!(torn.validate().unwrap_err().code(), "ERR_OCR_INVALID_IMAGE");
            let error = expect_error(
                recognizer
                    .process_image(&torn, &RecognitionOptions::default())
                    .await,
            );
            assert_eq!(error.code(), "ERR_OCR_INVALID_IMAGE");
        }
        assert_eq!(fake.started(), 0);
    });
}

#[test]
fn empty_encoded_bytes_are_refused_as_an_invalid_image() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        let empty = ImageInput::Encoded(Vec::new());
        assert_eq!(
            empty.validate().unwrap_err().code(),
            "ERR_OCR_INVALID_IMAGE"
        );
        let error = expect_error(
            recognizer
                .process_image(&empty, &RecognitionOptions::default())
                .await,
        );
        assert_eq!(error.code(), "ERR_OCR_INVALID_IMAGE");
        assert_eq!(fake.started(), 0);
        // Non-empty bytes pass validation and reach the backend.
        let call = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.pending()).await;
        assert!(fake.finish_recognition(Ok(sample_text("decoded"))));
        assert_eq!(call.await.unwrap().unwrap().text, "decoded");
    });
}

#[test]
fn every_kind_reaches_the_caller_with_its_code() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        for (kind, code) in ALL_KINDS {
            let call = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
            settle(|| fake.pending()).await;
            assert!(fake.finish_recognition(Err(OcrError::new(kind))));
            assert_eq!(expect_error(call.await.unwrap()).code(), code);
        }
    });
}

#[test]
fn every_kind_has_a_distinct_code() {
    let codes: Vec<&'static str> = ALL_KINDS
        .iter()
        .map(|(kind, _)| OcrError::new(*kind).code())
        .collect();
    let mut unique = codes.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(codes.len(), unique.len());
    for (kind, code) in ALL_KINDS {
        assert_eq!(OcrError::new(kind).code(), code);
        assert!(!kind.message().is_empty());
    }
}

#[test]
fn a_platform_status_is_kept_with_its_error() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        let call = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.pending()).await;
        let failure = OcrError::new(ErrorKind::Backend).with_status(-7);
        assert!(fake.finish_recognition(Err(failure)));
        let error = expect_error(call.await.unwrap());
        assert_eq!(error.kind(), ErrorKind::Backend);
        assert_eq!(error.status(), Some(-7));
    });
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn an_unsupported_script_resolves_with_the_unsupported_language_code() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        // The struct is non-exhaustive, so fields are set one by one.
        let mut options = RecognitionOptions::default();
        options.script = Script::Korean;
        fake.fail_next_recognition(OcrError::new(ErrorKind::UnsupportedLanguage));
        let error = expect_error(recognizer.process_image(&image(), &options).await);
        assert_eq!(error.code(), "ERR_OCR_UNSUPPORTED_LANGUAGE");
        assert_eq!(
            fake.started(),
            0,
            "nothing starts when the script is unsupported"
        );
        // The refusal does not poison the recognizer: the next call runs.
        let call = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.pending()).await;
        assert!(fake.finish_recognition(Ok(sample_text("runs"))));
        assert_eq!(call.await.unwrap().unwrap().text, "runs");
    });
}

#[test]
fn a_backend_engine_failure_resolves_with_the_backend_code() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        let call = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.pending()).await;
        assert!(fake.finish_recognition(Err(OcrError::new(ErrorKind::Backend))));
        assert_eq!(expect_error(call.await.unwrap()).code(), "ERR_OCR_BACKEND");
    });
}

#[test]
fn a_model_error_resolves_with_the_model_code() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        fake.fail_next_recognition(OcrError::new(ErrorKind::ModelNotLoaded));
        let error = expect_error(
            recognizer
                .process_image(&image(), &RecognitionOptions::default())
                .await,
        );
        assert_eq!(error.code(), "ERR_OCR_MODEL_NOT_LOADED");
        assert_eq!(fake.started(), 0, "without models nothing starts");
        let call = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.pending()).await;
        assert!(fake.finish_recognition(Ok(sample_text("runs"))));
        assert_eq!(call.await.unwrap().unwrap().text, "runs");
    });
}
