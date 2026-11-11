/** The codes a recognition rejection carries. Apps match on `code`. */
export type OcrErrorCode =
  | "ERR_OCR_NOT_AVAILABLE"
  | "ERR_OCR_UNSUPPORTED_LANGUAGE"
  | "ERR_OCR_MODEL_NOT_LOADED"
  | "ERR_OCR_INVALID_IMAGE"
  | "ERR_OCR_INVALID_ARGUMENT"
  | "ERR_OCR_IO_FAILED"
  | "ERR_OCR_CLOSED"
  | "ERR_OCR_CANCELLED"
  | "ERR_OCR_TIMEOUT"
  | "ERR_OCR_BACKEND";

/** Same wording as the Rust crate's messages. */
export const MESSAGES: Record<OcrErrorCode, string> = {
  ERR_OCR_NOT_AVAILABLE: "Text recognition is not available on this device.",
  ERR_OCR_UNSUPPORTED_LANGUAGE: "This script is not supported by this recognizer.",
  ERR_OCR_MODEL_NOT_LOADED: "The recognition model files were not provided or are incompatible.",
  ERR_OCR_INVALID_IMAGE: "The image is empty, unreadable, or has the wrong shape.",
  ERR_OCR_INVALID_ARGUMENT: "An argument is empty or not allowed on this platform.",
  ERR_OCR_IO_FAILED: "The image file could not be read.",
  ERR_OCR_CLOSED: "This recognizer is closed.",
  ERR_OCR_CANCELLED: "The recognition was cancelled.",
  ERR_OCR_TIMEOUT: "The recognition timed out. Try again.",
  ERR_OCR_BACKEND: "The platform's recognition engine failed.",
};

const KNOWN = new Set<string>(Object.keys(MESSAGES));

/** A rejection of the OCR API. `code` is the stable name; the message says what to do. */
export class OcrError extends Error {
  readonly code: OcrErrorCode;

  constructor(code: OcrErrorCode) {
    super(MESSAGES[code]);
    this.name = "OcrError";
    this.code = code;
  }
}

/** The error for a known code. */
export function ocrError(code: OcrErrorCode): OcrError {
  return new OcrError(code);
}

/**
 * Turns whatever the native side or a host threw into a coded `OcrError`. A host's rejection
 * carries `ERR_NSH_HOST_ERROR`, and its message starts with the code, as `ERR_OCR_CANCELLED:
 * text`. Anything unknown is a backend failure.
 */
export function asOcrError(error: unknown): OcrError {
  if (error instanceof OcrError) return error;
  const raw = (error ?? {}) as { code?: unknown; message?: unknown };
  const code = typeof raw.code === "string" ? raw.code : "";
  const message = typeof raw.message === "string" ? raw.message : "";
  if (code === "ERR_NSH_HOST_ERROR") {
    const named = /^(ERR_OCR_[A-Z_]+): /.exec(message)?.[1];
    return named !== undefined && KNOWN.has(named)
      ? ocrError(named as OcrErrorCode)
      : ocrError("ERR_OCR_BACKEND");
  }
  return KNOWN.has(code) ? ocrError(code as OcrErrorCode) : ocrError("ERR_OCR_BACKEND");
}
