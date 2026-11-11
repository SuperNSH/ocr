//! Windows: text recognition through `Windows.Media.Ocr`, one engine per language.
//!
//! Every WinRT call runs on the blocking pool and joins its `IAsyncOperation`, so nothing waits
//! on the engine thread. A started `RecognizeAsync` cannot be stopped: cancel is a no-op here
//! and the core drops the late answer.

// API surface verified against windows 0.62.2 and windows-future 0.3.2, both pinned in
// Cargo.toml; `join` is unconditional there - `std` follows the biometrics pin.

use std::panic::{AssertUnwindSafe, catch_unwind};

use windows::{
    Globalization::Language,
    Graphics::Imaging::{BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, SoftwareBitmap},
    Media::Ocr::{OcrEngine, OcrResult},
    Storage::Streams::{DataWriter, InMemoryRandomAccessStream},
    core::HSTRING,
};

use crate::{
    backend::{Backend, BackendFuture, RecognitionDone},
    error::{ErrorKind, OcrError},
    input::ImageInput,
    options::RecognitionOptions,
    text::{Point, RecognizedText, Rect, TextBlock, TextElement, TextLine},
};

pub(crate) struct NativeBackend;

/// A WinRT failure while the engine works: `ERR_OCR_BACKEND`, HRESULT in `status()`.
fn backend_error<T>(result: windows::core::Result<T>) -> Result<T, OcrError> {
    result.map_err(|error| OcrError::new(ErrorKind::Backend).with_status(i64::from(error.code().0)))
}

/// Bytes Windows cannot decode are a shape problem, not an engine failure.
fn image_error<T>(result: windows::core::Result<T>) -> Result<T, OcrError> {
    result.map_err(|error| {
        OcrError::new(ErrorKind::InvalidImage).with_status(i64::from(error.code().0))
    })
}

/// The recognized shapes, extracted first so the mapping below stays a pure function.
struct RawLine {
    text: String,
    words: Vec<RawWord>,
}

struct RawWord {
    text: String,
    rect: Rect,
}

/// Length is a multiple of four: `ImageInput::validate` already enforced
/// `data.len() == width * height * 4`, so `chunks_exact` never drops a pixel.
fn rgba_to_bgra(rgba: &[u8]) -> Vec<u8> {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]])
        .collect()
}

/// `OcrWord` boxes are f32 pixels from the top-left; round to the nearest integer edge.
fn int_rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect {
        left: x.round() as i32,
        top: y.round() as i32,
        right: (x + width).round() as i32,
        bottom: (y + height).round() as i32,
    }
}

/// The four corners, clockwise from the top-left.
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

/// The smallest rect covering every given rect; an empty slice gives the zero rect.
fn union_rects(rects: &[Rect]) -> Rect {
    rects
        .iter()
        .fold(None, |covering: Option<Rect>, rect| {
            Some(match covering {
                None => *rect,
                Some(c) => Rect {
                    left: c.left.min(rect.left),
                    top: c.top.min(rect.top),
                    right: c.right.max(rect.right),
                    bottom: c.bottom.max(rect.bottom),
                },
            })
        })
        .unwrap_or(Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        })
}

/// One Windows result becomes one block: Windows reports lines and words, no blocks of its own.
fn assemble_result(text: String, language: &str, lines: Vec<RawLine>) -> RecognizedText {
    if lines.is_empty() {
        return RecognizedText {
            text,
            blocks: Vec::new(),
        };
    }
    let block_text = lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let languages = vec![language.to_owned()];
    let mapped: Vec<TextLine> = lines
        .into_iter()
        .map(|line| {
            let boxes: Vec<Rect> = line.words.iter().map(|word| word.rect).collect();
            let bounding_box = union_rects(&boxes);
            let corners = corner_points(&bounding_box);
            TextLine {
                text: line.text,
                elements: line
                    .words
                    .into_iter()
                    .map(|word| {
                        let corners = corner_points(&word.rect);
                        TextElement {
                            text: word.text,
                            symbols: Vec::new(),
                            bounding_box: word.rect,
                            languages: languages.clone(),
                            corner_points: corners,
                            confidence: None,
                            angle: None,
                        }
                    })
                    .collect(),
                bounding_box,
                languages: languages.clone(),
                corner_points: corners,
                confidence: None,
                angle: None,
            }
        })
        .collect();
    let block_boxes: Vec<Rect> = mapped.iter().map(|line| line.bounding_box).collect();
    let block_box = union_rects(&block_boxes);
    let block_corners = corner_points(&block_box);
    let block = TextBlock {
        text: block_text,
        lines: mapped,
        bounding_box: block_box,
        languages,
        corner_points: block_corners,
    };
    RecognizedText {
        text,
        blocks: vec![block],
    }
}

/// The BCP-47 tags of the recognizers installed for the user.
fn recognizer_languages() -> Result<Vec<String>, OcrError> {
    let installed = backend_error(OcrEngine::AvailableRecognizerLanguages())?;
    let mut tags = Vec::new();
    for i in 0..backend_error(installed.Size())? {
        let language = backend_error(installed.GetAt(i))?;
        tags.push(backend_error(language.LanguageTag())?.to_string());
    }
    Ok(tags)
}

/// `RecognizeAsync` reports oversized images as an engine failure; refuse them first as a shape
/// error instead.
fn check_dimensions(width: i64, height: i64) -> Result<(), OcrError> {
    let max = i64::from(backend_error(OcrEngine::MaxImageDimension())?);
    if width > max || height > max {
        return Err(OcrError::new(ErrorKind::InvalidImage));
    }
    Ok(())
}

/// Encoded bytes become a Bgra8 bitmap through the decoder chain; the decoder refuses anything
/// it cannot read, which is `ERR_OCR_INVALID_IMAGE`.
fn decode_bitmap(bytes: &[u8]) -> Result<SoftwareBitmap, OcrError> {
    let stream = backend_error(InMemoryRandomAccessStream::new())?;
    let output = backend_error(stream.GetOutputStreamAt(0))?;
    let writer = backend_error(DataWriter::CreateDataWriter(&output))?;
    backend_error(writer.WriteBytes(bytes))?;
    // StoreAsync moves the bytes into the stream; the decoder reads the stream, not the writer.
    backend_error(backend_error(writer.StoreAsync())?.join())?;
    backend_error(writer.DetachStream())?;
    backend_error(stream.Seek(0))?;
    let operation = image_error(BitmapDecoder::CreateAsync(&stream))?;
    let decoder = image_error(operation.join())?;
    let operation = image_error(decoder.GetSoftwareBitmapConvertedAsync(
        BitmapPixelFormat::Bgra8,
        BitmapAlphaMode::Premultiplied,
    ))?;
    let bitmap = image_error(operation.join())?;
    check_dimensions(
        i64::from(backend_error(bitmap.PixelWidth())?),
        i64::from(backend_error(bitmap.PixelHeight())?),
    )?;
    Ok(bitmap)
}

/// Validated RGBA becomes a Bgra8 bitmap: Windows samples the bytes in BGRA order.
fn bitmap_from_rgba(width: u32, height: u32, data: &[u8]) -> Result<SoftwareBitmap, OcrError> {
    let width = i32::try_from(width).map_err(|_| OcrError::new(ErrorKind::InvalidImage))?;
    let height = i32::try_from(height).map_err(|_| OcrError::new(ErrorKind::InvalidImage))?;
    check_dimensions(i64::from(width), i64::from(height))?;
    let bitmap = backend_error(SoftwareBitmap::Create(
        BitmapPixelFormat::Bgra8,
        width,
        height,
    ))?;
    // The writer's buffer still holds the bytes written; hand it to the bitmap without storing.
    let stream = backend_error(InMemoryRandomAccessStream::new())?;
    let output = backend_error(stream.GetOutputStreamAt(0))?;
    let writer = backend_error(DataWriter::CreateDataWriter(&output))?;
    backend_error(writer.WriteBytes(&rgba_to_bgra(data)))?;
    let buffer = backend_error(writer.DetachBuffer())?;
    backend_error(SoftwareBitmap::CopyFromBuffer(&bitmap, &buffer))?;
    Ok(bitmap)
}

/// The whole chain, run on one blocking-pool thread. The language is refused before any pixel
/// work, so an unsupported script never pays for the bitmap.
fn recognize(image: &ImageInput, language_tag: &str) -> Result<RecognizedText, OcrError> {
    let language = backend_error(Language::CreateLanguage(&HSTRING::from(language_tag)))?;
    if !backend_error(OcrEngine::IsLanguageSupported(&language))? {
        return Err(OcrError::new(ErrorKind::UnsupportedLanguage));
    }
    let engine = backend_error(OcrEngine::TryCreateFromLanguage(&language))?;
    let bitmap = match image {
        ImageInput::Encoded(bytes) => decode_bitmap(bytes)?,
        ImageInput::Rgba {
            width,
            height,
            data,
        } => bitmap_from_rgba(*width, *height, data)?,
    };
    let operation = backend_error(engine.RecognizeAsync(&bitmap))?;
    let result = backend_error(operation.join())?;
    let engine_language = backend_error(engine.RecognizerLanguage())?;
    let tag = backend_error(engine_language.LanguageTag())?.to_string();
    Ok(assemble_result(
        backend_error(result.Text())?.to_string(),
        &tag,
        extract_lines(&result)?,
    ))
}

fn extract_lines(result: &OcrResult) -> Result<Vec<RawLine>, OcrError> {
    let win_lines = backend_error(result.Lines())?;
    let mut lines = Vec::new();
    for i in 0..backend_error(win_lines.Size())? {
        let win_line = backend_error(win_lines.GetAt(i))?;
        let text = backend_error(win_line.Text())?.to_string();
        let win_words = backend_error(win_line.Words())?;
        let mut words = Vec::new();
        for j in 0..backend_error(win_words.Size())? {
            let win_word = backend_error(win_words.GetAt(j))?;
            let rect = backend_error(win_word.BoundingRect())?;
            words.push(RawWord {
                text: backend_error(win_word.Text())?.to_string(),
                rect: int_rect(rect.X, rect.Y, rect.Width, rect.Height),
            });
        }
        lines.push(RawLine { text, words });
    }
    Ok(lines)
}

impl Backend for NativeBackend {
    fn supported_languages(&self) -> BackendFuture<'_, Result<Vec<String>, OcrError>> {
        Box::pin(async {
            nsh_core::prelude::blocking(recognizer_languages)
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
        let image = image.clone();
        let language = options.script.primary_language().to_owned();
        // `done` must fire even if the worker panics, or the caller would wait forever.
        let work = move || {
            let answer = catch_unwind(AssertUnwindSafe(|| recognize(&image, &language)))
                .unwrap_or_else(|_| Err(OcrError::new(ErrorKind::Backend)));
            done(answer);
        };
        // The returned handle is a future; dropping it detaches the worker.
        drop(nsh_core::prelude::blocking(work));
        Ok(())
    }

    fn cancel_recognition(&self) {
        // Windows.Media.Ocr cannot stop a running recognition; the late answer still arrives
        // through `done` and the core drops it.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_to_bgra_swaps_red_and_blue_and_keeps_alpha() {
        assert_eq!(rgba_to_bgra(&[]), Vec::<u8>::new());
        assert_eq!(rgba_to_bgra(&[1, 2, 3, 4]), vec![3, 2, 1, 4]);
        assert_eq!(
            rgba_to_bgra(&[1, 2, 3, 4, 250, 100, 5, 6]),
            vec![3, 2, 1, 4, 5, 100, 250, 6]
        );
    }

    #[test]
    fn int_rect_rounds_word_boxes_to_integers() {
        assert_eq!(
            int_rect(10.4, 20.6, 5.5, 6.5),
            Rect {
                left: 10,
                top: 21,
                right: 16,
                bottom: 27
            }
        );
    }

    #[test]
    fn corner_points_walk_the_box_clockwise() {
        let rect = Rect {
            left: 1,
            top: 2,
            right: 3,
            bottom: 4,
        };
        assert_eq!(
            corner_points(&rect),
            vec![
                Point { x: 1, y: 2 },
                Point { x: 3, y: 2 },
                Point { x: 3, y: 4 },
                Point { x: 1, y: 4 },
            ]
        );
    }

    #[test]
    fn union_rects_covers_every_rect_and_starts_at_zero() {
        assert_eq!(
            union_rects(&[
                Rect {
                    left: 0,
                    top: 0,
                    right: 2,
                    bottom: 2,
                },
                Rect {
                    left: 5,
                    top: 3,
                    right: 7,
                    bottom: 9,
                },
            ]),
            Rect {
                left: 0,
                top: 0,
                right: 7,
                bottom: 9
            }
        );
        assert_eq!(
            union_rects(&[]),
            Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0
            }
        );
        assert_eq!(
            union_rects(&[Rect {
                left: 1,
                top: 1,
                right: 2,
                bottom: 2,
            }]),
            Rect {
                left: 1,
                top: 1,
                right: 2,
                bottom: 2
            }
        );
    }

    #[test]
    fn no_lines_map_to_no_blocks() {
        let result = assemble_result(String::new(), "en-US", Vec::new());
        assert_eq!(result.text, "");
        assert!(result.blocks.is_empty());
    }

    #[test]
    fn lines_map_without_confidence_or_angle() {
        let first = RawLine {
            text: "hello world".to_owned(),
            words: vec![
                RawWord {
                    text: "hello".to_owned(),
                    rect: Rect {
                        left: 0,
                        top: 0,
                        right: 10,
                        bottom: 8,
                    },
                },
                RawWord {
                    text: "world".to_owned(),
                    rect: Rect {
                        left: 12,
                        top: 0,
                        right: 22,
                        bottom: 8,
                    },
                },
            ],
        };
        let second = RawLine {
            text: "second".to_owned(),
            words: vec![RawWord {
                text: "second".to_owned(),
                rect: Rect {
                    left: 0,
                    top: 20,
                    right: 30,
                    bottom: 28,
                },
            }],
        };
        let result = assemble_result(
            "hello world\r\nsecond".to_owned(),
            "en-US",
            vec![first, second],
        );
        assert_eq!(result.blocks.len(), 1);
        let block = &result.blocks[0];
        assert_eq!(result.text, "hello world\r\nsecond");
        assert_eq!(block.text, "hello world\nsecond");
        assert_eq!(block.languages, vec!["en-US".to_owned()]);
        assert_eq!(
            block.bounding_box,
            Rect {
                left: 0,
                top: 0,
                right: 30,
                bottom: 28
            }
        );
        assert_eq!(block.corner_points.len(), 4);
        assert_eq!(block.lines.len(), 2);
        let line = &block.lines[0];
        assert_eq!(line.confidence, None);
        assert_eq!(line.angle, None);
        assert_eq!(line.languages, vec!["en-US".to_owned()]);
        assert_eq!(
            line.bounding_box,
            Rect {
                left: 0,
                top: 0,
                right: 22,
                bottom: 8
            }
        );
        assert_eq!(line.corner_points.len(), 4);
        assert_eq!(line.elements.len(), 2);
        let element = &line.elements[0];
        assert_eq!(element.text, "hello");
        assert!(element.symbols.is_empty());
        assert_eq!(element.confidence, None);
        assert_eq!(element.angle, None);
        assert_eq!(element.languages, vec!["en-US".to_owned()]);
        assert_eq!(
            element.bounding_box,
            Rect {
                left: 0,
                top: 0,
                right: 10,
                bottom: 8
            }
        );
        assert_eq!(element.corner_points.len(), 4);
    }
}
