//! ocrs text-recognition backend for Linux and wasm32.
//!
// Pinned in Cargo.toml: ocrs 0.14.0, rten 0.28.0, image 0.25 (png + jpeg). rten with
// default-features = false needs a model-format feature: "rten_format" for .rten bytes.

use std::sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError};

use ocrs::{
    ImageSource, OcrEngine, OcrEngineParams, TextChar, TextItem, TextLine as OcrsTextLine, TextWord,
};

use crate::backend::{Backend, BackendFuture, RecognitionDone};
use crate::error::{ErrorKind, OcrError};
use crate::input::ImageInput;
use crate::options::{RecognitionOptions, Script};
use crate::text::{Point, RecognizedText, Rect, TextBlock, TextElement, TextLine, TextSymbol};

/// The stock ocrs models recognise Latin script only.
static LANGUAGES: LazyLock<Vec<String>> = LazyLock::new(|| vec!["en".to_string()]);

static MODEL_BYTES: Mutex<Option<ModelBytes>> = Mutex::new(None);

struct ModelBytes {
    detection: Vec<u8>,
    recognition: Vec<u8>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panicking recognition must not poison model access for later calls.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn model_not_loaded() -> OcrError {
    OcrError::new(ErrorKind::ModelNotLoaded)
}

fn invalid_image() -> OcrError {
    OcrError::new(ErrorKind::InvalidImage)
}

fn engine_failure() -> OcrError {
    OcrError::new(ErrorKind::Backend)
}

// Every ocrs engine failure (model mismatch, inference failure) surfaces as ERR_OCR_BACKEND.
impl From<ocrs::OcrError> for OcrError {
    fn from(_: ocrs::OcrError) -> Self {
        engine_failure()
    }
}

fn parse_model(bytes: &[u8]) -> Result<rten::Model, OcrError> {
    // Model::load takes owned bytes and auto-detects the .rten/.onnx format.
    rten::Model::load(bytes.to_vec()).map_err(|_| model_not_loaded())
}

fn build_engine(detection: &[u8], recognition: &[u8]) -> Result<OcrEngine, OcrError> {
    OcrEngine::new(OcrEngineParams {
        detection_model: Some(parse_model(detection)?),
        recognition_model: Some(parse_model(recognition)?),
        ..OcrEngineParams::default()
    })
    .map_err(|_| model_not_loaded())
}

fn refuse_non_latin(options: &RecognitionOptions) -> Result<(), OcrError> {
    if options.script != Script::Latin {
        // The stock models are Latin-only; refuse before any work starts.
        return Err(OcrError::new(ErrorKind::UnsupportedLanguage));
    }
    Ok(())
}

/// Text-recognition backend running ocrs with caller-supplied model bytes.
pub struct OcrsBackend {
    engine: Arc<OcrEngine>,
}

impl OcrsBackend {
    /// Creates a backend from detection and recognition model bytes. Both models
    /// are parsed eagerly; unreadable or incompatible bytes are
    /// [`ErrorKind::ModelNotLoaded`].
    pub fn from_model_bytes(detection: Vec<u8>, recognition: Vec<u8>) -> Result<Self, OcrError> {
        let engine = build_engine(&detection, &recognition)?;
        Ok(Self {
            engine: Arc::new(engine),
        })
    }
}

impl Backend for OcrsBackend {
    fn supported_languages(&self) -> BackendFuture<'_, Result<Vec<String>, OcrError>> {
        Box::pin(async { Ok(LANGUAGES.clone()) })
    }

    fn start_recognition(
        &self,
        image: &ImageInput,
        options: &RecognitionOptions,
        done: RecognitionDone,
    ) -> Result<(), OcrError> {
        refuse_non_latin(options)?;
        deliver(Arc::clone(&self.engine), image.clone(), done);
        Ok(())
    }

    fn cancel_recognition(&self) {
        // Inference cannot be interrupted; the core drops late answers after a cancel.
    }
}

/// Stores the model bytes the default backend uses; a later call replaces them.
/// The bytes are parsed now so broken models fail here, not at first recognition.
pub fn use_ocrs_models(detection: Vec<u8>, recognition: Vec<u8>) -> Result<(), OcrError> {
    build_engine(&detection, &recognition)?;
    *lock(&MODEL_BYTES) = Some(ModelBytes {
        detection,
        recognition,
    });
    Ok(())
}

/// Default Linux/wasm32 backend: lazily builds one engine from the bytes
/// registered through [`use_ocrs_models`].
#[derive(Default)]
pub(crate) struct NativeBackend {
    engine: Mutex<Option<Arc<OcrEngine>>>,
}

impl NativeBackend {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn engine(&self) -> Result<Arc<OcrEngine>, OcrError> {
        if let Some(engine) = lock(&self.engine).as_ref() {
            return Ok(Arc::clone(engine));
        }
        let bytes = lock(&MODEL_BYTES)
            .as_ref()
            .map(|stored| (stored.detection.clone(), stored.recognition.clone()));
        let Some((detection, recognition)) = bytes else {
            return Err(model_not_loaded());
        };
        let built = Arc::new(build_engine(&detection, &recognition)?);
        let mut cached = lock(&self.engine);
        if let Some(existing) = cached.as_ref() {
            // Two first calls can race the build; reuse whichever engine landed first.
            return Ok(Arc::clone(existing));
        }
        *cached = Some(Arc::clone(&built));
        Ok(built)
    }
}

impl Backend for NativeBackend {
    fn supported_languages(&self) -> BackendFuture<'_, Result<Vec<String>, OcrError>> {
        // Answers without models: the stock models only ever recognise English.
        Box::pin(async { Ok(LANGUAGES.clone()) })
    }

    fn start_recognition(
        &self,
        image: &ImageInput,
        options: &RecognitionOptions,
        done: RecognitionDone,
    ) -> Result<(), OcrError> {
        refuse_non_latin(options)?;
        let engine = self.engine()?;
        deliver(engine, image.clone(), done);
        Ok(())
    }

    fn cancel_recognition(&self) {
        // Inference cannot be interrupted; the core drops late answers after a cancel.
    }
}

/// Answers 'done' exactly once even when inference panics (ocrs 0.14 has reachable
/// panics); AssertUnwindSafe is sound because the torn state is never reused.
fn answer(engine: &OcrEngine, image: &ImageInput, done: RecognitionDone) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_recognition(engine, image)
    }));
    done(match outcome {
        Ok(result) => result,
        Err(_panic) => Err(engine_failure()),
    });
}

#[cfg(target_arch = "wasm32")]
fn deliver(engine: Arc<OcrEngine>, image: ImageInput, done: RecognitionDone) {
    // No threads on wasm32: run inline; 'done' still fires exactly once, synchronously.
    answer(&engine, &image, done);
}

#[cfg(not(target_arch = "wasm32"))]
fn deliver(engine: Arc<OcrEngine>, image: ImageInput, done: RecognitionDone) {
    // Builder::spawn consumes its closure even on failure, so the job sits in a
    // shared slot: exactly one of the worker or the inline fallback resolves 'done'.
    let job = Arc::new(Mutex::new(Some((engine, image, done))));
    let worker = {
        let job = Arc::clone(&job);
        move || {
            if let Some((engine, image, done)) = lock(&job).take() {
                answer(&engine, &image, done);
            }
        }
    };
    if std::thread::Builder::new()
        .name("nsh-ocr-recognition".to_string())
        .spawn(worker)
        .is_err()
    {
        // Thread exhaustion: resolve inline so the call never hangs.
        if let Some((engine, image, done)) = lock(&job).take() {
            answer(&engine, &image, done);
        }
    }
}

fn run_recognition(engine: &OcrEngine, image: &ImageInput) -> Result<RecognizedText, OcrError> {
    let input = prepare_input(engine, image)?;
    let words = engine.detect_words(&input)?;
    let line_rects = engine.find_text_lines(&input, &words);
    let lines = engine.recognize_text(&input, &line_rects)?;
    let mapped: Vec<TextLine> = lines.iter().flatten().map(map_ocrs_line).collect();
    // '\n'-joining the recognized lines' texts is exactly what get_text computes.
    let text = mapped
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    build_recognized_text(mapped, text)
}

fn prepare_input(engine: &OcrEngine, image: &ImageInput) -> Result<ocrs::OcrInput, OcrError> {
    match image {
        ImageInput::Encoded(bytes) => {
            let decoded = decode_encoded(bytes)?;
            let source = ImageSource::from_bytes(decoded.as_raw(), decoded.dimensions())
                .map_err(|_| invalid_image())?;
            Ok(engine.prepare_input(source)?)
        }
        ImageInput::Rgba {
            width,
            height,
            data,
        } => {
            let source =
                ImageSource::from_bytes(data, (*width, *height)).map_err(|_| invalid_image())?;
            Ok(engine.prepare_input(source)?)
        }
    }
}

fn decode_encoded(bytes: &[u8]) -> Result<image::RgbImage, OcrError> {
    image::load_from_memory(bytes)
        .map(|decoded| decoded.into_rgb8())
        .map_err(|_| invalid_image())
}

fn map_ocrs_line(line: &OcrsTextLine) -> TextLine {
    let elements: Vec<TextElement> = line.words().map(|word| map_word(&word)).collect();
    let bounding_box = union_all(elements.iter().map(|element| &element.bounding_box))
        .unwrap_or_else(|| {
            // A line of only spaces yields no words; use the union of its char rects.
            let chars = line.bounding_rect();
            map_rect(chars.top(), chars.left(), chars.bottom(), chars.right())
        });
    let corner_points = elements
        .iter()
        .flat_map(|element| element.corner_points.iter())
        .map(|point| Point {
            x: point.x,
            y: point.y,
        })
        .collect();
    TextLine {
        text: line.to_string(),
        elements,
        bounding_box,
        languages: LANGUAGES.clone(),
        corner_points,
        confidence: None,
        angle: None,
    }
}

fn map_word(word: &TextWord<'_>) -> TextElement {
    let rect = word.bounding_rect();
    TextElement {
        text: word.to_string(),
        symbols: word.chars().iter().map(map_char).collect(),
        bounding_box: map_rect(rect.top(), rect.left(), rect.bottom(), rect.right()),
        languages: LANGUAGES.clone(),
        corner_points: rect
            .corners()
            .into_iter()
            .map(|point| Point {
                x: point.x,
                y: point.y,
            })
            .collect(),
        confidence: None,
        angle: None,
    }
}

fn map_char(character: &TextChar) -> TextSymbol {
    TextSymbol {
        text: character.char.to_string(),
        bounding_box: map_rect(
            character.rect.top(),
            character.rect.left(),
            character.rect.bottom(),
            character.rect.right(),
        ),
        languages: LANGUAGES.clone(),
        corner_points: character
            .rect
            .corners()
            .into_iter()
            .map(|point| Point {
                x: point.x,
                y: point.y,
            })
            .collect(),
        confidence: None,
        angle: None,
    }
}

/// ocrs geometry is top/left/bottom/right integers, the same edges our Rect stores.
fn map_rect(top: i32, left: i32, bottom: i32, right: i32) -> Rect {
    Rect {
        left,
        top,
        right,
        bottom,
    }
}

fn union_rect(a: &Rect, b: &Rect) -> Rect {
    Rect {
        left: a.left.min(b.left),
        top: a.top.min(b.top),
        right: a.right.max(b.right),
        bottom: a.bottom.max(b.bottom),
    }
}

/// Union of rects. Union with itself copies the first rect, so no Rect clone is needed.
fn union_all(rects: impl Iterator<Item = &Rect>) -> Option<Rect> {
    let mut union: Option<Rect> = None;
    for rect in rects {
        union = Some(match union {
            Some(current) => union_rect(&current, rect),
            None => union_rect(rect, rect),
        });
    }
    union
}

/// Groups the recognized lines under one TextBlock; ocrs has no block concept.
/// 'text' is the '\n'-joined line texts, identical to OcrEngine::get_text's output.
fn build_recognized_text(lines: Vec<TextLine>, text: String) -> RecognizedText {
    let Some(bounding_box) = union_all(lines.iter().map(|line| &line.bounding_box)) else {
        // No lines means no text found: success with empty text and no blocks.
        return RecognizedText {
            text,
            blocks: Vec::new(),
        };
    };
    let corner_points = lines
        .iter()
        .flat_map(|line| line.corner_points.iter())
        .map(|point| Point {
            x: point.x,
            y: point.y,
        })
        .collect();
    RecognizedText {
        text: text.clone(),
        blocks: vec![TextBlock {
            text,
            lines,
            bounding_box,
            languages: LANGUAGES.clone(),
            corner_points,
        }],
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::{
        LANGUAGES, NativeBackend, OcrsBackend, build_recognized_text, decode_encoded, map_rect,
        union_all, use_ocrs_models,
    };
    use crate::backend::{Backend, RecognitionDone};
    use crate::error::ErrorKind;
    use crate::input::ImageInput;
    use crate::options::{RecognitionOptions, Script};
    use crate::text::{Point, Rect, TextElement, TextLine};

    // 2x1 RGB PNG (red then green pixel), authored outside the image crate.
    const TINY_PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 1, 8, 2,
        0, 0, 0, 123, 64, 232, 221, 0, 0, 0, 15, 73, 68, 65, 84, 120, 218, 99, 248, 207, 192, 192,
        240, 159, 1, 0, 7, 255, 1, 255, 184, 4, 53, 224, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
        130,
    ];

    // No test stores valid models, so the process-wide MODEL_BYTES stays empty and
    // the model-less assertions below hold under any test ordering.
    fn rgba() -> ImageInput {
        ImageInput::Rgba {
            width: 1,
            height: 1,
            data: vec![0, 0, 0, 255],
        }
    }

    fn options(script: Script) -> RecognitionOptions {
        RecognitionOptions {
            script,
            timeout_ms: None,
        }
    }

    /// Returns a done-callback plus a flag recording whether it ever fired.
    fn tracking_done() -> (RecognitionDone, Arc<AtomicBool>) {
        let fired = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&fired);
        (Box::new(move |_| flag.store(true, Ordering::SeqCst)), fired)
    }

    fn corner_points(rect: &Rect) -> Vec<Point> {
        let Rect {
            left,
            top,
            right,
            bottom,
        } = *rect;
        vec![
            Point { x: left, y: top },
            Point { x: right, y: top },
            Point {
                x: right,
                y: bottom,
            },
            Point { x: left, y: bottom },
        ]
    }

    fn element(text: &str, bounding_box: Rect) -> TextElement {
        TextElement {
            text: text.to_string(),
            symbols: Vec::new(),
            languages: LANGUAGES.clone(),
            corner_points: corner_points(&bounding_box),
            bounding_box,
            confidence: None,
            angle: None,
        }
    }

    fn line(text: &str, bounding_box: Rect, elements: Vec<TextElement>) -> TextLine {
        TextLine {
            text: text.to_string(),
            elements,
            languages: LANGUAGES.clone(),
            corner_points: corner_points(&bounding_box),
            bounding_box,
            confidence: None,
            angle: None,
        }
    }

    #[test]
    fn map_rect_keeps_tlbr_edges() {
        let rect = map_rect(10, 5, 30, 25);
        assert_eq!(
            rect,
            Rect {
                left: 5,
                top: 10,
                right: 25,
                bottom: 30
            }
        );
    }

    #[test]
    fn union_all_spans_every_rect() {
        let rects = [
            Rect {
                left: 0,
                top: 0,
                right: 10,
                bottom: 8,
            },
            Rect {
                left: 5,
                top: 4,
                right: 20,
                bottom: 12,
            },
        ];
        let union = union_all(rects.iter()).unwrap();
        assert_eq!(
            union,
            Rect {
                left: 0,
                top: 0,
                right: 20,
                bottom: 12
            }
        );
        assert!(union_all(std::iter::empty()).is_none());
    }

    #[test]
    fn build_recognized_text_groups_lines_under_one_block() {
        let first = line(
            "hello world",
            Rect {
                left: 0,
                top: 0,
                right: 40,
                bottom: 10,
            },
            vec![element(
                "hello",
                Rect {
                    left: 0,
                    top: 0,
                    right: 20,
                    bottom: 10,
                },
            )],
        );
        let second = line(
            "again",
            Rect {
                left: 2,
                top: 12,
                right: 30,
                bottom: 22,
            },
            Vec::new(),
        );
        let result = build_recognized_text(vec![first, second], "hello world\nagain".to_string());
        assert_eq!(result.text, "hello world\nagain");
        assert_eq!(result.blocks.len(), 1);
        let block = &result.blocks[0];
        assert_eq!(block.text, "hello world\nagain");
        assert_eq!(block.lines.len(), 2);
        assert_eq!(
            block.bounding_box,
            Rect {
                left: 0,
                top: 0,
                right: 40,
                bottom: 22
            }
        );
        assert_eq!(block.languages, vec!["en".to_string()]);
        assert_eq!(block.corner_points.len(), 8);
        let first_line = &block.lines[0];
        assert_eq!(first_line.elements.len(), 1);
        assert_eq!(first_line.elements[0].text, "hello");
        assert_eq!(first_line.corner_points.len(), 4);
    }

    #[test]
    fn build_recognized_text_without_lines_is_empty_success() {
        let result = build_recognized_text(Vec::new(), String::new());
        assert_eq!(result.text, "");
        assert!(result.blocks.is_empty());
    }

    #[test]
    fn decode_encoded_reads_png_bytes() {
        let decoded = decode_encoded(TINY_PNG).unwrap();
        assert_eq!(decoded.dimensions(), (2, 1));
        assert_eq!(decoded.as_raw(), &[255, 0, 0, 0, 255, 0]);
    }

    #[test]
    fn decode_encoded_rejects_garbage_as_invalid_image() {
        let error = decode_encoded(&[1, 2, 3, 4]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidImage);
    }

    #[test]
    fn garbage_model_bytes_fail_with_model_not_loaded() {
        let error = OcrsBackend::from_model_bytes(b"junk".to_vec(), b"junk".to_vec()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ModelNotLoaded);
    }

    #[test]
    fn use_ocrs_models_rejects_garbage_and_stores_nothing() {
        let error = use_ocrs_models(b"junk".to_vec(), b"junk".to_vec()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ModelNotLoaded);
        let (done, fired) = tracking_done();
        let error = NativeBackend::new()
            .start_recognition(&rgba(), &options(Script::Latin), done)
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ModelNotLoaded);
        assert!(!fired.load(Ordering::SeqCst));
    }

    #[test]
    fn a_non_latin_script_is_refused_before_any_work_starts() {
        let (done, fired) = tracking_done();
        let error = NativeBackend::new()
            .start_recognition(&rgba(), &options(Script::Japanese), done)
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::UnsupportedLanguage);
        assert!(!fired.load(Ordering::SeqCst));
    }

    #[test]
    fn native_backend_without_models_reports_model_not_loaded() {
        let (done, fired) = tracking_done();
        let error = NativeBackend::new()
            .start_recognition(&rgba(), &options(Script::Latin), done)
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ModelNotLoaded);
        assert!(!fired.load(Ordering::SeqCst));
    }

    #[test]
    fn supported_languages_answer_english_without_models() {
        let backend = NativeBackend::new();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let languages = runtime.block_on(backend.supported_languages()).unwrap();
        assert_eq!(languages, vec!["en".to_string()]);
    }
}
