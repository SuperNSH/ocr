//! The shapes that come back: the result tree, the script names, and what reaches the backend.

mod common;

use common::{expect_error, image, run, sample_text, settle, spawn_process};
use nsh_ocr::{
    ErrorKind, FakeBackend, OcrError, Point, RecognitionOptions, RecognizedText, Rect, Script,
    TextBlock, TextElement, TextLine, TextRecognizer, TextSymbol,
};

#[test]
fn the_result_tree_mirrors_the_reference_shapes() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        let call = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.pending()).await;
        assert!(fake.finish_recognition(Ok(reference_tree())));
        let got = call.await.unwrap().unwrap();

        assert_eq!(got.text, "Hello, OCR");
        let block = got.blocks.first().unwrap();
        assert_eq!(block.text, "Hello, OCR");
        assert_eq!(block.languages, vec!["en-US".to_owned()]);
        assert_eq!(block.corner_points.len(), 4);
        assert_eq!(block.bounding_box.left, 4);
        assert_eq!(block.bounding_box.top, 8);
        assert_eq!(block.bounding_box.right, 96);
        assert_eq!(block.bounding_box.bottom, 20);

        let line = block.lines.first().unwrap();
        assert_eq!(line.text, "Hello, OCR");
        assert_eq!(line.confidence, Some(0.9));
        assert_eq!(line.angle, Some(-0.25));
        assert_eq!(line.corner_points.first().unwrap().y, 8);
        assert_eq!(line.corner_points.last().unwrap().y, 20);

        let element = line.elements.first().unwrap();
        assert_eq!(element.text, "Hello,");
        assert_eq!(element.confidence, Some(0.95));
        assert_eq!(
            element.angle, None,
            "angle is Some only where the platform gives one"
        );

        let symbol = element.symbols.first().unwrap();
        assert_eq!(symbol.text, "Hello,");
        assert_eq!(symbol.languages, vec!["en-US".to_owned()]);
        assert_eq!(symbol.confidence, Some(0.91));
        assert_eq!(symbol.angle, Some(0.5));
        assert_eq!(symbol.bounding_box.right, 40);
        assert_eq!(symbol.corner_points.len(), 4);
    });
}

#[test]
fn each_script_has_the_name_tsx_uses() {
    let scripts: [(Script, &str, &str); 5] = [
        (Script::Latin, "latin", "en-US"),
        (Script::Chinese, "chinese", "zh-Hans"),
        (Script::Devanagiri, "devanagari", "hi-IN"),
        (Script::Japanese, "japanese", "ja-JP"),
        (Script::Korean, "korean", "ko-KR"),
    ];
    for (script, name, language) in scripts {
        assert_eq!(script.name(), name);
        assert_eq!(script.primary_language(), language);
    }
    assert_eq!(Script::default(), Script::Latin);
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn the_script_and_options_reach_the_backend() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        // The struct is non-exhaustive, so fields are set one by one.
        let mut options = RecognitionOptions::default();
        options.script = Script::Japanese;
        options.timeout_ms = Some(60_000);
        let call = spawn_process(&recognizer, &image(), &options);
        settle(|| fake.pending()).await;
        assert_eq!(fake.last_image(), Some(image()));
        assert_eq!(fake.last_options(), Some(options));
        assert!(fake.finish_recognition(Ok(sample_text("日本語"))));
        assert_eq!(call.await.unwrap().unwrap().text, "日本語");
    });
}

#[test]
fn supported_languages_pass_through_the_backend() {
    let fake = FakeBackend::new();
    fake.set_supported_languages(Ok(vec!["ja-JP".to_owned(), "ko-KR".to_owned()]));
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        assert_eq!(
            recognizer.supported_languages().await.unwrap(),
            vec!["ja-JP".to_owned(), "ko-KR".to_owned()]
        );
        fake.set_supported_languages(Err(OcrError::new(ErrorKind::NotAvailable)));
        let error = expect_error(recognizer.supported_languages().await);
        assert_eq!(error.code(), "ERR_OCR_NOT_AVAILABLE");
    });
}

#[test]
fn an_image_with_no_text_resolves_with_empty_text_and_no_blocks() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        let call = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.pending()).await;
        assert!(fake.finish_recognition(Ok(RecognizedText {
            text: String::new(),
            blocks: Vec::new(),
        })));
        let empty = call.await.unwrap().unwrap();
        assert!(empty.text.is_empty());
        assert!(empty.blocks.is_empty());
    });
}

#[test]
fn unicode_text_passes_through_unchanged() {
    let fake = FakeBackend::new();
    let recognizer = TextRecognizer::with_backend(fake.clone());
    run(async {
        let call = spawn_process(&recognizer, &image(), &RecognitionOptions::default());
        settle(|| fake.pending()).await;
        assert!(fake.finish_recognition(Ok(unicode_tree())));
        let got = call.await.unwrap().unwrap();
        assert_eq!(got.text, "こんにちは、世界 — café naïve Ω 📷");
        let block = got.blocks.first().unwrap();
        assert_eq!(block.text, "こんにちは、世界");
        let line = block.lines.first().unwrap();
        assert_eq!(line.text, "こんにちは、世界");
        let element = line.elements.first().unwrap();
        assert_eq!(element.text, "こんにちは、世界");
        let symbol = element.symbols.first().unwrap();
        assert_eq!(symbol.text, "こんにちは、世界");
        assert_eq!(symbol.languages, vec!["ja-JP".to_owned()]);
    });
}

/// The reference shape: every level present, confidence and angle set only where a platform gives them.
fn reference_tree() -> RecognizedText {
    let word_box = Rect {
        left: 4,
        top: 8,
        right: 40,
        bottom: 20,
    };
    let line_box = Rect {
        left: 4,
        top: 8,
        right: 96,
        bottom: 20,
    };
    let word_corners = vec![
        Point { x: 4, y: 8 },
        Point { x: 40, y: 8 },
        Point { x: 40, y: 20 },
        Point { x: 4, y: 20 },
    ];
    RecognizedText {
        text: "Hello, OCR".to_owned(),
        blocks: vec![TextBlock {
            text: "Hello, OCR".to_owned(),
            lines: vec![TextLine {
                text: "Hello, OCR".to_owned(),
                elements: vec![TextElement {
                    text: "Hello,".to_owned(),
                    symbols: vec![TextSymbol {
                        text: "Hello,".to_owned(),
                        bounding_box: word_box,
                        languages: vec!["en-US".to_owned()],
                        corner_points: word_corners.clone(),
                        confidence: Some(0.91),
                        angle: Some(0.5),
                    }],
                    bounding_box: word_box,
                    languages: vec!["en-US".to_owned()],
                    corner_points: word_corners.clone(),
                    confidence: Some(0.95),
                    angle: None,
                }],
                bounding_box: line_box,
                languages: vec!["en-US".to_owned()],
                corner_points: word_corners.clone(),
                confidence: Some(0.9),
                angle: Some(-0.25),
            }],
            bounding_box: line_box,
            languages: vec!["en-US".to_owned()],
            corner_points: word_corners,
        }],
    }
}

/// One block, one line, one element, one symbol, all carrying the same Unicode text.
fn unicode_tree() -> RecognizedText {
    let label = "こんにちは、世界".to_owned();
    let box_ = Rect {
        left: 0,
        top: 0,
        right: 42,
        bottom: 12,
    };
    RecognizedText {
        text: format!("{label} — café naïve Ω 📷"),
        blocks: vec![TextBlock {
            text: label.clone(),
            lines: vec![TextLine {
                text: label.clone(),
                elements: vec![TextElement {
                    text: label.clone(),
                    symbols: vec![TextSymbol {
                        text: label.clone(),
                        bounding_box: box_,
                        languages: vec!["ja-JP".to_owned()],
                        corner_points: Vec::new(),
                        confidence: None,
                        angle: None,
                    }],
                    bounding_box: box_,
                    languages: vec!["ja-JP".to_owned()],
                    corner_points: Vec::new(),
                    confidence: None,
                    angle: None,
                }],
                bounding_box: box_,
                languages: vec!["ja-JP".to_owned()],
                corner_points: Vec::new(),
                confidence: None,
                angle: None,
            }],
            bounding_box: box_,
            languages: vec!["ja-JP".to_owned()],
            corner_points: Vec::new(),
        }],
    }
}
