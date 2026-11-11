//! The errors a recognition call reports. Each has a stable code; a message says what to do.

use std::fmt;

/// What went wrong, as a stable code; see [`OcrError::code`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// Text recognition does not exist on this device: no engine, or a target with none.
    NotAvailable,
    /// The script the options name is not one this recognizer can read.
    UnsupportedLanguage,
    /// The recognition model files were not provided or are incompatible.
    ModelNotLoaded,
    /// The image is empty, unreadable, or has the wrong shape.
    InvalidImage,
    /// An argument is empty or not allowed on this platform.
    InvalidArgument,
    /// The image file could not be read.
    IoFailed,
    /// This recognizer is closed; every later call on it refuses.
    Closed,
    /// The user, the app or the system cancelled the recognition.
    Cancelled,
    /// The call gave up waiting: the timeout passed with the answer still missing.
    Timeout,
    /// The platform's recognition engine failed; `status` holds its own code where it gives one.
    Backend,
}

/// A failed recognition call or language query.
///
/// `Display` names the code and the message. Neither holds the image or any user data.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}", .kind.message())]
pub struct OcrError {
    kind: ErrorKind,
    status: Option<i64>,
}

impl OcrError {
    /// An error of `kind`, with no platform status.
    pub fn new(kind: ErrorKind) -> Self {
        Self { kind, status: None }
    }

    /// The same error, carrying the platform's own status code.
    #[must_use]
    pub fn with_status(mut self, status: i64) -> Self {
        self.status = Some(status);
        self
    }

    /// What went wrong.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The platform's status code, when the platform gave one.
    pub fn status(&self) -> Option<i64> {
        self.status
    }

    /// The stable code, such as `ERR_OCR_CANCELLED`. Apps match on this.
    pub fn code(&self) -> &'static str {
        self.kind.code()
    }
}

impl ErrorKind {
    /// The stable code of this kind.
    pub fn code(self) -> &'static str {
        match self {
            ErrorKind::NotAvailable => "ERR_OCR_NOT_AVAILABLE",
            ErrorKind::UnsupportedLanguage => "ERR_OCR_UNSUPPORTED_LANGUAGE",
            ErrorKind::ModelNotLoaded => "ERR_OCR_MODEL_NOT_LOADED",
            ErrorKind::InvalidImage => "ERR_OCR_INVALID_IMAGE",
            ErrorKind::InvalidArgument => "ERR_OCR_INVALID_ARGUMENT",
            ErrorKind::IoFailed => "ERR_OCR_IO_FAILED",
            ErrorKind::Closed => "ERR_OCR_CLOSED",
            ErrorKind::Cancelled => "ERR_OCR_CANCELLED",
            ErrorKind::Timeout => "ERR_OCR_TIMEOUT",
            ErrorKind::Backend => "ERR_OCR_BACKEND",
        }
    }

    /// What to do about it, in one sentence.
    pub fn message(self) -> &'static str {
        match self {
            ErrorKind::NotAvailable => "Text recognition is not available on this device.",
            ErrorKind::UnsupportedLanguage => "This script is not supported by this recognizer.",
            ErrorKind::ModelNotLoaded => {
                "The recognition model files were not provided or are incompatible."
            }
            ErrorKind::InvalidImage => "The image is empty, unreadable, or has the wrong shape.",
            ErrorKind::InvalidArgument => "An argument is empty or not allowed on this platform.",
            ErrorKind::IoFailed => "The image file could not be read.",
            ErrorKind::Closed => "This recognizer is closed.",
            ErrorKind::Cancelled => "The recognition was cancelled.",
            ErrorKind::Timeout => "The recognition timed out. Try again.",
            ErrorKind::Backend => "The platform's recognition engine failed.",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [(ErrorKind, &str, &str); 10] = [
        (
            ErrorKind::NotAvailable,
            "ERR_OCR_NOT_AVAILABLE",
            "Text recognition is not available on this device.",
        ),
        (
            ErrorKind::UnsupportedLanguage,
            "ERR_OCR_UNSUPPORTED_LANGUAGE",
            "This script is not supported by this recognizer.",
        ),
        (
            ErrorKind::ModelNotLoaded,
            "ERR_OCR_MODEL_NOT_LOADED",
            "The recognition model files were not provided or are incompatible.",
        ),
        (
            ErrorKind::InvalidImage,
            "ERR_OCR_INVALID_IMAGE",
            "The image is empty, unreadable, or has the wrong shape.",
        ),
        (
            ErrorKind::InvalidArgument,
            "ERR_OCR_INVALID_ARGUMENT",
            "An argument is empty or not allowed on this platform.",
        ),
        (
            ErrorKind::IoFailed,
            "ERR_OCR_IO_FAILED",
            "The image file could not be read.",
        ),
        (
            ErrorKind::Closed,
            "ERR_OCR_CLOSED",
            "This recognizer is closed.",
        ),
        (
            ErrorKind::Cancelled,
            "ERR_OCR_CANCELLED",
            "The recognition was cancelled.",
        ),
        (
            ErrorKind::Timeout,
            "ERR_OCR_TIMEOUT",
            "The recognition timed out. Try again.",
        ),
        (
            ErrorKind::Backend,
            "ERR_OCR_BACKEND",
            "The platform's recognition engine failed.",
        ),
    ];

    #[test]
    fn every_kind_has_its_frozen_code_and_message() {
        for (kind, code, message) in ALL {
            assert_eq!(kind.code(), code);
            assert_eq!(kind.message(), message);
            assert_eq!(OcrError::new(kind).code(), code);
        }
    }

    #[test]
    fn every_kind_has_a_distinct_code() {
        let codes: Vec<&'static str> = ALL.iter().map(|(kind, _)| kind.code()).collect();
        let mut unique = codes.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(codes.len(), unique.len());
    }

    #[test]
    fn display_prints_the_code() {
        assert_eq!(ErrorKind::Cancelled.to_string(), "ERR_OCR_CANCELLED");
        assert_eq!(
            OcrError::new(ErrorKind::Timeout).to_string(),
            "The recognition timed out. Try again."
        );
    }

    #[test]
    fn a_status_travels_with_its_error() {
        let error = OcrError::new(ErrorKind::Backend).with_status(-7);
        assert_eq!(error.kind(), ErrorKind::Backend);
        assert_eq!(error.status(), Some(-7));
        assert_eq!(OcrError::new(ErrorKind::Backend).status(), None);
    }
}
