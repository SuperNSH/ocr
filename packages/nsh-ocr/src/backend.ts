import type { ImageInput } from "./input";
import type { RecognitionOptions } from "./options";
import type { RecognizedText } from "./text";

/** What a `TextRecognizer` runs on: the Rust crate, or a memory backend in tests. */
export interface OcrBackend {
  /** The BCP-47 languages this backend can recognise now. */
  supportedLanguages(): Promise<string[]>;
  /** Runs one recognition. Resolves once, with the text the image held. */
  processImage(image: ImageInput, options: RecognitionOptions): Promise<RecognizedText>;
  /** Asks the backend to stop. Resolves `true` when a recognition was running. */
  cancel(): Promise<boolean>;
  /** Releases the backend's recognizer; its later calls refuse with `ERR_OCR_CLOSED`. */
  close(): Promise<void>;
}
