//! The recognition as TSX sees it: `NativeTextRecognizer`, a class whose async members reject
//! with a `code` on failure. Results are plain values; the recognizer stays on the Rust side.
//!
//! Binary fields (`encoded`, `rgba.data`) cross as Uint8Array: `Vec<u8>` is the bridge value
//! nsh-core defines for bytes (runtime `FileData`), so no Base64 strings anywhere.

use nsh_core::prelude::*;

use crate::{
    ErrorKind, ImageInput, OcrError, Point, RecognitionOptions, RecognizedText, Rect, Script,
    TextBlock, TextElement, TextLine, TextRecognizer, TextSymbol,
};

/// Raw RGBA8 pixels, row-major, top-left origin; `data.len()` must equal `width * height * 4`.
#[derive(JsObject, Clone, Debug, Default)]
pub struct NativeRgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// The image to recognise text in: a `path` to a file, `encoded` bytes, or `rgba` pixels;
/// exactly one of the three.
#[derive(JsObject, Clone, Debug, Default)]
pub struct NativeImageInput {
    pub path: Option<String>,
    pub encoded: Option<Vec<u8>>,
    pub rgba: Option<NativeRgba>,
}

/// The options of one recognition, from TSX. Unset fields keep the defaults.
#[derive(JsObject, Clone, Debug, Default)]
pub struct NativeRecognitionOptions {
    /// One of `Script::name`'s strings: `"latin"`, `"chinese"`, `"devanagari"`, `"japanese"`, `"korean"`.
    pub script: Option<String>,
    /// The longest the call waits, in milliseconds. Off unless set; unused on the web.
    pub timeout_ms: Option<u64>,
}

/// One recognition's whole answer, as TSX receives it.
#[derive(JsObject, Clone, Debug, Default)]
pub struct NativeRecognizedText {
    pub text: String,
    pub blocks: Vec<NativeTextBlock>,
}

/// A paragraph-sized group of lines.
#[derive(JsObject, Clone, Debug, Default)]
pub struct NativeTextBlock {
    pub text: String,
    pub lines: Vec<NativeTextLine>,
    pub bounding_box: Rect,
    pub languages: Vec<String>,
    pub corner_points: Vec<Point>,
}

/// One line of text; `confidence` and `angle` are `Some` only where the platform gives them.
#[derive(JsObject, Clone, Debug, Default)]
pub struct NativeTextLine {
    pub text: String,
    pub elements: Vec<NativeTextElement>,
    pub bounding_box: Rect,
    pub languages: Vec<String>,
    pub corner_points: Vec<Point>,
    pub confidence: Option<f32>,
    pub angle: Option<f32>,
}

/// One word of a line; `confidence` and `angle` are `Some` only where the platform gives them.
#[derive(JsObject, Clone, Debug, Default)]
pub struct NativeTextElement {
    pub text: String,
    pub symbols: Vec<NativeTextSymbol>,
    pub bounding_box: Rect,
    pub languages: Vec<String>,
    pub corner_points: Vec<Point>,
    pub confidence: Option<f32>,
    pub angle: Option<f32>,
}

/// One character of an element; `confidence` and `angle` are `Some` only where the platform
/// gives them.
#[derive(JsObject, Clone, Debug, Default)]
pub struct NativeTextSymbol {
    pub text: String,
    pub bounding_box: Rect,
    pub languages: Vec<String>,
    pub corner_points: Vec<Point>,
    pub confidence: Option<f32>,
    pub angle: Option<f32>,
}

/// A recognizer. Create one and keep it; every handle shares the platform's backend.
#[class]
pub struct NativeTextRecognizer {
    recognizer: TextRecognizer,
}

#[class]
impl NativeTextRecognizer {
    /// The platform's text recognition.
    pub fn new() -> NativeTextRecognizer {
        NativeTextRecognizer {
            recognizer: TextRecognizer::new(),
        }
    }

    /// The BCP-47 languages this recognizer can read now.
    pub async fn supported_languages(this: Instance<Self>) -> Result<Vec<String>, JsError> {
        let recognizer = this.with(|native| native.recognizer.clone());
        recognizer.supported_languages().await.map_err(into_js)
    }

    /// Recognises text in `image`. An image with no text resolves with empty text and no blocks.
    pub async fn process_image(
        this: Instance<Self>,
        image: NativeImageInput,
        options: Option<NativeRecognitionOptions>,
    ) -> Result<NativeRecognizedText, JsError> {
        let recognizer = this.with(|native| native.recognizer.clone());
        let image = to_image_input(image).map_err(into_js)?;
        let options = to_options(options.unwrap_or_default()).map_err(into_js)?;
        let text = recognizer
            .process_image(&image, &options)
            .await
            .map_err(into_js)?;
        Ok(map_text(&text))
    }

    /// Cancels every pending recognition. Resolves `true` when any call was pending.
    pub async fn cancel(this: Instance<Self>) -> Result<bool, JsError> {
        let recognizer = this.with(|native| native.recognizer.clone());
        // cancel and close are infallible on the core; only their value crosses.
        Ok(recognizer.cancel().await)
    }

    /// Closes the recognizer; every later call rejects with `ERR_OCR_CLOSED`.
    pub async fn close(this: Instance<Self>) -> Result<(), JsError> {
        let recognizer = this.with(|native| native.recognizer.clone());
        Ok(recognizer.close().await)
    }
}

impl Default for NativeTextRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

/// The error a rejection carries: its message, and the `code` apps match on.
fn into_js(error: OcrError) -> JsError {
    JsError::Error(error.kind().message().to_owned()).with_code(error.code())
}

const SCRIPTS: [Script; 5] = [
    Script::Latin,
    Script::Chinese,
    Script::Devanagiri,
    Script::Japanese,
    Script::Korean,
];

/// The script a TSX name stands for; the names accepted are exactly `Script::name`'s.
fn script_from_name(name: &str) -> Option<Script> {
    SCRIPTS.iter().copied().find(|script| script.name() == name)
}

/// Turns the TSX input into the crate's. A `path` is read here; a read failure is
/// [`crate::ErrorKind::IoFailed`]. More than one variant set is an invalid argument.
fn to_image_input(image: NativeImageInput) -> Result<ImageInput, OcrError> {
    match (image.path, image.encoded, image.rgba) {
        (Some(path), None, None) => std::fs::read(&path)
            .map(ImageInput::Encoded)
            .map_err(|_| OcrError::new(ErrorKind::IoFailed)),
        (None, Some(encoded), None) => Ok(ImageInput::Encoded(encoded)),
        (None, None, Some(rgba)) => Ok(ImageInput::Rgba {
            width: rgba.width,
            height: rgba.height,
            data: rgba.data,
        }),
        _ => Err(OcrError::new(ErrorKind::InvalidArgument)),
    }
}

/// Turns the TSX options into the crate's, refusing a script name it does not know.
fn to_options(options: NativeRecognitionOptions) -> Result<RecognitionOptions, OcrError> {
    let script = match options.script.as_deref() {
        None => Script::default(),
        Some(name) => {
            script_from_name(name).ok_or_else(|| OcrError::new(ErrorKind::InvalidArgument))?
        }
    };
    Ok(RecognitionOptions {
        script,
        timeout_ms: options.timeout_ms,
    })
}

/// Maps the core's answer to the TSX one; the geometry passes through unchanged.
fn map_text(text: &RecognizedText) -> NativeRecognizedText {
    NativeRecognizedText {
        text: text.text.clone(),
        blocks: text.blocks.iter().map(map_block).collect(),
    }
}

fn map_block(block: &TextBlock) -> NativeTextBlock {
    NativeTextBlock {
        text: block.text.clone(),
        lines: block.lines.iter().map(map_line).collect(),
        bounding_box: block.bounding_box,
        languages: block.languages.clone(),
        corner_points: block.corner_points.clone(),
    }
}

fn map_line(line: &TextLine) -> NativeTextLine {
    NativeTextLine {
        text: line.text.clone(),
        elements: line.elements.iter().map(map_element).collect(),
        bounding_box: line.bounding_box,
        languages: line.languages.clone(),
        corner_points: line.corner_points.clone(),
        confidence: line.confidence,
        angle: line.angle,
    }
}

fn map_element(element: &TextElement) -> NativeTextElement {
    NativeTextElement {
        text: element.text.clone(),
        symbols: element.symbols.iter().map(map_symbol).collect(),
        bounding_box: element.bounding_box,
        languages: element.languages.clone(),
        corner_points: element.corner_points.clone(),
        confidence: element.confidence,
        angle: element.angle,
    }
}

fn map_symbol(symbol: &TextSymbol) -> NativeTextSymbol {
    NativeTextSymbol {
        text: symbol.text.clone(),
        bounding_box: symbol.bounding_box,
        languages: symbol.languages.clone(),
        corner_points: symbol.corner_points.clone(),
        confidence: symbol.confidence,
        angle: symbol.angle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_script_name_parses_and_an_unknown_one_is_refused() {
        for script in SCRIPTS {
            assert_eq!(script_from_name(script.name()), Some(script));
        }
        assert_eq!(script_from_name("cyrillic"), None);
        let error = to_options(NativeRecognitionOptions {
            script: Some("cyrillic".to_owned()),
            timeout_ms: None,
        })
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    }

    #[test]
    fn unset_options_keep_the_defaults_and_set_ones_are_kept() {
        assert_eq!(
            to_options(NativeRecognitionOptions::default()).ok(),
            Some(RecognitionOptions::default())
        );
        let options = to_options(NativeRecognitionOptions {
            script: Some("japanese".to_owned()),
            timeout_ms: Some(5_000),
        })
        .unwrap();
        assert_eq!(options.script, Script::Japanese);
        assert_eq!(options.timeout_ms, Some(5_000));
    }

    #[test]
    fn an_input_is_exactly_one_variant() {
        let encoded = to_image_input(NativeImageInput {
            path: None,
            encoded: Some(vec![1, 2]),
            rgba: None,
        })
        .unwrap();
        assert_eq!(encoded, ImageInput::Encoded(vec![1, 2]));
        let rgba = to_image_input(NativeImageInput {
            path: None,
            encoded: None,
            rgba: Some(NativeRgba {
                width: 1,
                height: 1,
                data: vec![0, 0, 0, 255],
            }),
        })
        .unwrap();
        assert_eq!(
            rgba,
            ImageInput::Rgba {
                width: 1,
                height: 1,
                data: vec![0, 0, 0, 255]
            }
        );
        let none = to_image_input(NativeImageInput::default()).unwrap_err();
        assert_eq!(none.kind(), ErrorKind::InvalidArgument);
        let both = to_image_input(NativeImageInput {
            path: Some("/tmp/x.png".to_owned()),
            encoded: Some(vec![1]),
            rgba: None,
        })
        .unwrap_err();
        assert_eq!(both.kind(), ErrorKind::InvalidArgument);
    }

    #[test]
    fn a_path_that_cannot_be_read_is_an_io_failure() {
        // A NUL byte is not a path character on any target, so the read fails everywhere.
        let error = to_image_input(NativeImageInput {
            path: Some("\0not-a-path".to_owned()),
            encoded: None,
            rgba: None,
        })
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::IoFailed);
    }

    #[test]
    fn the_tree_maps_level_for_level() {
        let text = RecognizedText {
            text: "hi".to_owned(),
            blocks: vec![TextBlock {
                text: "hi".to_owned(),
                lines: vec![TextLine {
                    text: "hi".to_owned(),
                    elements: vec![TextElement {
                        text: "hi".to_owned(),
                        symbols: vec![TextSymbol {
                            text: "h".to_owned(),
                            bounding_box: Rect {
                                left: 0,
                                top: 1,
                                right: 2,
                                bottom: 3,
                            },
                            languages: vec!["en-US".to_owned()],
                            corner_points: vec![Point { x: 0, y: 1 }],
                            confidence: Some(0.9),
                            angle: None,
                        }],
                        bounding_box: Rect {
                            left: 0,
                            top: 1,
                            right: 4,
                            bottom: 3,
                        },
                        languages: vec!["en-US".to_owned()],
                        corner_points: Vec::new(),
                        confidence: None,
                        angle: Some(1.5),
                    }],
                    bounding_box: Rect {
                        left: 0,
                        top: 1,
                        right: 4,
                        bottom: 3,
                    },
                    languages: vec!["en-US".to_owned()],
                    corner_points: Vec::new(),
                    confidence: Some(0.8),
                    angle: None,
                }],
                bounding_box: Rect {
                    left: 0,
                    top: 1,
                    right: 4,
                    bottom: 3,
                },
                languages: vec!["en-US".to_owned()],
                corner_points: Vec::new(),
            }],
        };
        let mapped = map_text(&text);
        assert_eq!(mapped.text, "hi");
        let block = &mapped.blocks[0];
        assert_eq!(block.text, "hi");
        assert_eq!(block.bounding_box.right, 4);
        let line = &block.lines[0];
        assert_eq!(line.confidence, Some(0.8));
        assert_eq!(line.angle, None);
        let element = &line.elements[0];
        assert_eq!(element.angle, Some(1.5));
        let symbol = &element.symbols[0];
        assert_eq!(symbol.text, "h");
        assert_eq!(symbol.bounding_box.bottom, 3);
        assert_eq!(symbol.confidence, Some(0.9));
        assert_eq!(symbol.corner_points, vec![Point { x: 0, y: 1 }]);
    }

    #[test]
    fn errors_carry_their_message_and_code() {
        let js = into_js(OcrError::new(ErrorKind::Closed));
        let JsError::Fields(fields) = &js else {
            panic!("the error carries its code as a field");
        };
        assert_eq!(fields.message, ErrorKind::Closed.message());
        assert_eq!(
            fields.field("code").and_then(|code| code.as_str()),
            Some("ERR_OCR_CLOSED")
        );
    }
}
