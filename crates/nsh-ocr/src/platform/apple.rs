//! Apple (iOS and macOS): Vision recognizes the text on a thread of its own, fed by a `CGImage`.
//!
//! Vision cannot stop a request that has started, so `cancel_recognition` does nothing and the
//! core drops the late answer, like biometrics' prompts.

// objc2-vision 0.3.2 [VNRecognizeTextRequest VNRequest VNRequestHandler VNTypes VNObservation objc2-core-foundation objc2-core-graphics] and objc2 0.6.5, verified on the docs.rs sources;
// the rest 0.3.2 + std: objc2-foundation [NSArray NSString NSDictionary NSError], objc2-core-foundation [CFData], objc2-core-graphics [CGImage CGDataProvider CGColorSpace], objc2-image-io [CGImageSource objc2-core-graphics].

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::thread;

use objc2::AnyThread;
use objc2_core_foundation::{CFData, CFRetained};
use objc2_core_graphics::{
    CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage, CGImageAlphaInfo,
};
use objc2_foundation::{NSArray, NSDictionary, NSString};
use objc2_image_io::CGImageSource;
use objc2_vision::{
    VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel,
};

use crate::{
    backend::{Backend, BackendFuture, RecognitionDone},
    error::{ErrorKind, OcrError},
    input::ImageInput,
    options::RecognitionOptions,
    text::{Point, RecognizedText, Rect, TextBlock, TextLine},
};

pub(crate) struct NativeBackend;

/// The languages Vision supports for the accurate level, asked of a fresh request.
fn supported_recognition_languages() -> Result<Vec<String>, OcrError> {
    let request = VNRecognizeTextRequest::new();
    request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
    // SAFETY: the method's only contract is the error convention; no memory-safety preconditions.
    unsafe { request.supportedRecognitionLanguagesAndReturnError() }
        .map(|languages| languages.to_vec().iter().map(|l| l.to_string()).collect())
        .map_err(|error| OcrError::new(ErrorKind::Backend).with_status(error.code() as i64))
}

/// Decodes the input into the image Vision reads: encoded bytes through an image source,
/// raw RGBA through a data provider.
fn cg_image(image: &ImageInput) -> Result<CFRetained<CGImage>, OcrError> {
    match image {
        ImageInput::Encoded(bytes) => {
            let data = CFData::from_bytes(bytes);
            // SAFETY: `data` is a CFData and no options are passed, so no generic can be wrong.
            let source = unsafe { CGImageSource::with_data(&data, None) }
                .ok_or_else(|| OcrError::new(ErrorKind::InvalidImage))?;
            // SAFETY: the source is a live image source; count takes no options.
            if unsafe { source.count() } == 0 {
                return Err(OcrError::new(ErrorKind::InvalidImage));
            }
            // SAFETY: index 0 exists because count() reported at least one image.
            unsafe { source.image_at_index(0, None) }
                .ok_or_else(|| OcrError::new(ErrorKind::InvalidImage))
        }
        ImageInput::Rgba {
            width,
            height,
            data,
        } => {
            let bytes = CFData::from_bytes(data);
            let provider = CGDataProvider::with_cf_data(Some(&bytes))
                .ok_or_else(|| OcrError::new(ErrorKind::InvalidImage))?;
            let space =
                CGColorSpace::new_device_rgb().ok_or_else(|| OcrError::new(ErrorKind::Backend))?;
            // Straight RGBA8: alpha last, default byte order. validate() guarantees
            // data.len() == width * height * 4, so the row length below cannot overflow.
            let bitmap = CGBitmapInfo::from_bits_retain(CGImageAlphaInfo::Last.0);
            // SAFETY: decode is null, and 8-8-8-8 with alpha last is a supported bitmap format.
            unsafe {
                CGImage::new(
                    *width as usize,
                    *height as usize,
                    8,
                    32,
                    *width as usize * 4,
                    Some(&space),
                    bitmap,
                    Some(&provider),
                    std::ptr::null(),
                    true,
                    CGColorRenderingIntent::RenderingIntentDefault,
                )
            }
            .ok_or_else(|| OcrError::new(ErrorKind::Backend))
        }
    }
}

/// Runs one recognition to its end; the script was checked against the supported list before
/// the thread started. Returns the answer the caller resolves with.
fn recognize(image: &ImageInput, language: &str) -> Result<RecognizedText, OcrError> {
    let cg = cg_image(image)?;
    let width = CGImage::width(Some(&cg)) as u32;
    let height = CGImage::height(Some(&cg)) as u32;
    let options = NSDictionary::new();
    // SAFETY: the options dictionary's generic is VNImageOption, as declared, and alloc()'s
    // uninitialized instance is fully initialized by the initializer.
    let handler = unsafe {
        VNImageRequestHandler::initWithCGImage_options(
            VNImageRequestHandler::alloc(),
            &cg,
            &options,
        )
    };
    let request = VNRecognizeTextRequest::new();
    request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
    let language_string = NSString::from_str(language);
    let language_ref: &NSString = &language_string;
    request.setRecognitionLanguages(&NSArray::from_slice(&[language_ref]));
    let request_ref: &VNRequest = &request;
    let requests = NSArray::from_slice(&[request_ref]);
    handler
        .performRequests_error(&requests)
        .map_err(|error| OcrError::new(ErrorKind::Backend).with_status(error.code() as i64))?;
    // SAFETY: the request is live; the getter's return type matches its declaration.
    let languages: Vec<String> = unsafe { request.recognitionLanguages() }
        .to_vec()
        .iter()
        .map(|l| l.to_string())
        .collect();
    let lines = request
        .results()
        .map_or_else(Vec::new, |observations| observations.to_vec())
        .iter()
        .filter_map(|observation| {
            let text = observation.topCandidates(1).firstObject()?;
            // SAFETY: the observation is a live rectangle observation; the getter is plain.
            let box_rect = unsafe { observation.boundingBox() };
            let bounding_box = pixel_rect(
                box_rect.origin.x,
                box_rect.origin.y,
                box_rect.size.width,
                box_rect.size.height,
                width,
                height,
            );
            Some(TextLine {
                text: text.string().to_string(),
                elements: Vec::new(),
                bounding_box,
                languages: languages.clone(),
                corner_points: corner_points(&bounding_box),
                confidence: Some(text.confidence()),
                angle: None,
            })
        })
        .collect();
    Ok(one_block(lines, languages))
}

/// Vision reports a box in normalized coordinates with the origin at the image's bottom-left
/// corner; pixel space has its origin at the top-left, so the y axis flips.
fn pixel_rect(x: f64, y: f64, w: f64, h: f64, width: u32, height: u32) -> Rect {
    Rect {
        left: (x * f64::from(width)).round() as i32,
        top: ((1.0 - y - h) * f64::from(height)).round() as i32,
        right: ((x + w) * f64::from(width)).round() as i32,
        bottom: ((1.0 - y) * f64::from(height)).round() as i32,
    }
}

/// The rectangle's corners, clockwise from the top-left.
fn corner_points(rect: &Rect) -> Vec<Point> {
    vec![
        Point {
            x: rect.left,
            y: rect.top,
        },
        Point {
            x: rect.right,
            y: rect.top,
        },
        Point {
            x: rect.right,
            y: rect.bottom,
        },
        Point {
            x: rect.left,
            y: rect.bottom,
        },
    ]
}

/// The smallest rectangle covering `rects`; an empty slice gives an empty rectangle.
fn union_rects(rects: &[Rect]) -> Rect {
    let mut covering = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let mut any = false;
    for rect in rects {
        if !any {
            covering = Rect {
                left: rect.left,
                top: rect.top,
                right: rect.right,
                bottom: rect.bottom,
            };
            any = true;
            continue;
        }
        covering.left = covering.left.min(rect.left);
        covering.top = covering.top.min(rect.top);
        covering.right = covering.right.max(rect.right);
        covering.bottom = covering.bottom.max(rect.bottom);
    }
    covering
}

/// The joined text: the lines' text separated by newlines, empty when nothing was found.
fn joined_text(texts: &[&str]) -> String {
    texts.join("\n")
}

/// Groups every line under one block, because Vision has no block concept: the block's box
/// covers all its lines and its text joins theirs.
fn one_block(lines: Vec<TextLine>, languages: Vec<String>) -> RecognizedText {
    if lines.is_empty() {
        return RecognizedText {
            text: String::new(),
            blocks: Vec::new(),
        };
    }
    let boxes = lines
        .iter()
        .map(|line| Rect {
            left: line.bounding_box.left,
            top: line.bounding_box.top,
            right: line.bounding_box.right,
            bottom: line.bounding_box.bottom,
        })
        .collect::<Vec<_>>();
    let text = joined_text(
        &lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>(),
    );
    let bounding_box = union_rects(&boxes);
    RecognizedText {
        text: text.clone(),
        blocks: vec![TextBlock {
            text,
            lines,
            bounding_box,
            languages,
            corner_points: corner_points(&bounding_box),
        }],
    }
}

impl Backend for NativeBackend {
    fn supported_languages(&self) -> BackendFuture<'_, Result<Vec<String>, OcrError>> {
        Box::pin(async {
            // Vision objects are !Send: they are made and dropped inside the blocking closure.
            nsh_core::prelude::blocking(supported_recognition_languages)
                .await
                .map_err(|_| OcrError::new(ErrorKind::Backend))?
        })
    }

    fn start_recognition(
        &self,
        image: &ImageInput,
        options: &RecognitionOptions,
        done: RecognitionDone,
    ) -> Result<(), OcrError> {
        // The script is resolved before anything starts, so an unsupported one starts no work.
        let language = options.script.primary_language();
        let supported = supported_recognition_languages()?;
        if !supported.iter().any(|item| item == language) {
            return Err(OcrError::new(ErrorKind::UnsupportedLanguage));
        }
        let image = image.clone();
        let language = language.to_owned();
        let work = move || {
            // A panic in Vision must still answer once, or the caller would hang forever.
            let answer = catch_unwind(AssertUnwindSafe(|| recognize(&image, &language)))
                .unwrap_or_else(|_| Err(OcrError::new(ErrorKind::Backend)));
            done(answer);
        };
        // Vision objects are !Send, so the request runs on a thread of its own.
        thread::Builder::new()
            .spawn(work)
            .map_err(|_| OcrError::new(ErrorKind::Backend))?;
        Ok(())
    }

    // Vision cannot stop a running request; the core drops the late answer after a cancel.
    fn cancel_recognition(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_normalized_box_maps_to_a_top_left_pixel_rect() {
        let rect = pixel_rect(0.1, 0.25, 0.5, 0.5, 100, 200);
        assert_eq!(
            (rect.left, rect.top, rect.right, rect.bottom),
            (10, 50, 60, 150)
        );
    }

    #[test]
    fn the_full_image_covers_every_pixel() {
        let rect = pixel_rect(0.0, 0.0, 1.0, 1.0, 100, 200);
        assert_eq!(
            (rect.left, rect.top, rect.right, rect.bottom),
            (0, 0, 100, 200)
        );
    }

    #[test]
    fn edges_round_to_the_nearest_pixel() {
        let rect = pixel_rect(0.125, 0.0, 0.5, 0.25, 100, 100);
        assert_eq!(
            (rect.left, rect.top, rect.right, rect.bottom),
            (13, 75, 63, 100)
        );
    }

    #[test]
    fn corners_go_clockwise_from_the_top_left() {
        let rect = Rect {
            left: 1,
            top: 2,
            right: 5,
            bottom: 7,
        };
        let points = corner_points(&rect);
        let flat: Vec<(i32, i32)> = points.iter().map(|p| (p.x, p.y)).collect();
        assert_eq!(flat, vec![(1, 2), (5, 2), (5, 7), (1, 7)]);
    }

    #[test]
    fn the_union_covers_every_rect() {
        let rects = [
            Rect {
                left: 0,
                top: 0,
                right: 10,
                bottom: 10,
            },
            Rect {
                left: 5,
                top: 5,
                right: 20,
                bottom: 30,
            },
        ];
        let union = union_rects(&rects);
        assert_eq!(
            (union.left, union.top, union.right, union.bottom),
            (0, 0, 20, 30)
        );
    }

    #[test]
    fn the_union_of_nothing_is_empty() {
        let union = union_rects(&[]);
        assert_eq!(
            (union.left, union.top, union.right, union.bottom),
            (0, 0, 0, 0)
        );
    }

    #[test]
    fn the_joined_text_separates_lines_with_newlines() {
        assert_eq!(joined_text(&["alpha", "beta"]), "alpha\nbeta");
        assert_eq!(joined_text(&[]), "");
    }

    fn line(text: &str, left: i32, top: i32, right: i32, bottom: i32) -> TextLine {
        let bounding_box = Rect {
            left,
            top,
            right,
            bottom,
        };
        TextLine {
            text: text.to_owned(),
            elements: Vec::new(),
            bounding_box,
            languages: vec!["en-US".to_owned()],
            corner_points: corner_points(&bounding_box),
            confidence: Some(0.5),
            angle: None,
        }
    }

    #[test]
    fn one_block_groups_the_lines_and_covers_them() {
        let lines = vec![line("alpha", 0, 0, 10, 10), line("beta", 5, 5, 20, 30)];
        let recognized = one_block(lines, vec!["en-US".to_owned()]);
        assert_eq!(recognized.text, "alpha\nbeta");
        let block = &recognized.blocks[0];
        assert_eq!(block.text, "alpha\nbeta");
        assert_eq!(block.languages, vec!["en-US".to_owned()]);
        assert_eq!(
            (
                block.bounding_box.left,
                block.bounding_box.top,
                block.bounding_box.right,
                block.bounding_box.bottom
            ),
            (0, 0, 20, 30)
        );
        assert_eq!(block.lines.len(), 2);
        assert_eq!(block.corner_points.len(), 4);
    }

    #[test]
    fn no_lines_mean_no_blocks_and_empty_text() {
        let recognized = one_block(Vec::new(), vec!["en-US".to_owned()]);
        assert_eq!(recognized.text, "");
        assert!(recognized.blocks.is_empty());
    }
}
