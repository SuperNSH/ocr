//! The image a recognition takes: encoded bytes, or raw RGBA pixels.

use crate::error::{ErrorKind, OcrError};

/// The image to recognise text in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageInput {
    /// Encoded image bytes in a format the platform decodes (PNG, JPEG, ...).
    Encoded(Vec<u8>),
    /// Raw RGBA8 pixels, row-major, top-left origin; `data.len()` must equal `width * height * 4`.
    Rgba {
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
}

impl ImageInput {
    /// Checks the image's shape. Encoded bytes only have to be non-empty here; whether the
    /// platform can decode them is the backend's to say, as [`ErrorKind::InvalidImage`].
    pub fn validate(&self) -> Result<(), OcrError> {
        match self {
            ImageInput::Encoded(bytes) if bytes.is_empty() => Err(invalid_image()),
            ImageInput::Encoded(_) => Ok(()),
            ImageInput::Rgba {
                width,
                height,
                data,
            } => {
                // u64 math: on a 32-bit target the usize product would overflow.
                if *width == 0 || *height == 0 {
                    return Err(invalid_image());
                }
                let expected = u64::from(*width) * u64::from(*height) * 4;
                if data.len() as u64 != expected {
                    return Err(invalid_image());
                }
                Ok(())
            }
        }
    }
}

fn invalid_image() -> OcrError {
    OcrError::new(ErrorKind::InvalidImage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_encoded_bytes_are_invalid() {
        assert_eq!(
            ImageInput::Encoded(Vec::new())
                .validate()
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidImage
        );
    }

    #[test]
    fn non_empty_encoded_bytes_pass() {
        assert_eq!(ImageInput::Encoded(vec![1, 2, 3, 4]).validate(), Ok(()));
    }

    #[test]
    fn zero_dimensions_are_invalid() {
        let torn = [
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
        ];
        for image in torn {
            assert_eq!(
                image.validate().unwrap_err().kind(),
                ErrorKind::InvalidImage
            );
        }
    }

    #[test]
    fn a_size_mismatch_is_invalid_and_a_match_passes() {
        let torn = ImageInput::Rgba {
            width: 2,
            height: 2,
            data: vec![0; 15],
        };
        assert_eq!(torn.validate().unwrap_err().kind(), ErrorKind::InvalidImage);
        let whole = ImageInput::Rgba {
            width: 2,
            height: 2,
            data: vec![0; 16],
        };
        assert_eq!(whole.validate(), Ok(()));
    }

    #[test]
    fn dimensions_large_enough_to_overflow_32_bit_usize_are_measured_in_u64() {
        let torn = ImageInput::Rgba {
            width: u32::MAX,
            height: u32::MAX,
            data: Vec::new(),
        };
        assert_eq!(torn.validate().unwrap_err().kind(), ErrorKind::InvalidImage);
    }
}
