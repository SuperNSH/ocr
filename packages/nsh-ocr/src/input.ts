import { ocrError } from "./errors";

/** Raw RGBA8 pixels, row-major, top-left origin; `data.length` must equal `width * height * 4`. */
export type RgbaImage = { width: number; height: number; data: Uint8Array };

/** An image to recognise: encoded bytes the platform decodes (PNG, JPEG, ...), raw pixels, or a file path. */
export type ImageInput = { path: string } | { encoded: Uint8Array } | RgbaImage;

/**
 * Refuses a shape no platform can read: an empty path, empty encoded bytes, a zero dimension or
 * a pixel-length mismatch, all with `ERR_OCR_INVALID_IMAGE`. Undecodable bytes still reach the
 * platform, which rejects them with the same code.
 */
export function validateImageInput(image: ImageInput): void {
  if ("path" in image) {
    if (image.path.trim() === "") throw ocrError("ERR_OCR_INVALID_IMAGE");
    return;
  }
  if ("encoded" in image) {
    if (image.encoded.length === 0) throw ocrError("ERR_OCR_INVALID_IMAGE");
    return;
  }
  if (!("width" in image && "height" in image && "data" in image)) throw ocrError("ERR_OCR_INVALID_IMAGE");
  // A NaN dimension fails the > 0 check, as a zero or negative one does.
  if (!(image.width > 0) || !(image.height > 0) || image.data.length !== image.width * image.height * 4) {
    throw ocrError("ERR_OCR_INVALID_IMAGE");
  }
}
