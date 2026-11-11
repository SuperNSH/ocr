//! Text recognition for Native Shell apps: Apple's Vision, Android's ML Kit, Windows'
//! `Media.Ocr` and ocrs on Linux and the web, under one API.
//!
//! [`TextRecognizer`] reads text from an image. Each call resolves exactly once: `Ok` with a
//! [`RecognizedText`], or an [`OcrError`] with a stable code.

mod backend;
mod error;
mod fake;
mod input;
mod native;
mod options;
mod platform;
mod recognizer;
mod text;

use nsh_core::prelude::*;

pub use backend::{Backend, BackendFuture, RecognitionDone};
pub use error::{ErrorKind, OcrError};
pub use fake::FakeBackend;
pub use input::ImageInput;
pub use native::{
    NativeImageInput, NativeRecognitionOptions, NativeRecognizedText, NativeTextBlock,
    NativeTextElement, NativeTextLine, NativeTextRecognizer, NativeTextSymbol,
};
pub use options::{RecognitionOptions, Script};
pub use recognizer::TextRecognizer;
pub use text::{Point, RecognizedText, Rect, TextBlock, TextElement, TextLine, TextSymbol};

/// The crate's UI set: what TSX imports from `nsh-ocr/native`.
#[native_module("nsh-ocr/native")]
fn native(registry: &mut Registry) {
    registry.class::<NativeTextRecognizer>();
}

/// The ocrs engine's public surface, on the targets that run it.
#[cfg(any(target_os = "linux", target_arch = "wasm32"))]
pub use platform::ocrs::{OcrsBackend, use_ocrs_models};
