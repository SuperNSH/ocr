export { TextRecognizer, type RecognizeArgs } from "./recognizer";
export { type OcrBackend } from "./backend";
export { OcrError, ocrError, asOcrError, type OcrErrorCode } from "./errors";
export { memoryBackend, type MemoryOcr, type MemoryOcrConfig } from "./memory";
export { type ImageInput, type RgbaImage } from "./input";
export { type Script, type RecognitionOptions, SCRIPT_LANGUAGES } from "./options";
export {
  type RecognizedText,
  type TextBlock,
  type TextLine,
  type TextElement,
  type TextSymbol,
  type Rect,
  type Point,
} from "./text";
export { rustBackend } from "./rust";
