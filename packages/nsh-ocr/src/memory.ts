import type { OcrBackend } from "./backend";
import { ocrError, type OcrErrorCode } from "./errors";
import type { ImageInput } from "./input";
import type { RecognitionOptions } from "./options";
import type { RecognizedText } from "./text";

/** The answers a memory backend gives: each query answers what the config says. */
export interface MemoryOcrConfig {
  /** The languages `supportedLanguages` answers, or the code it rejects with. */
  supportedLanguages?: string[] | OcrErrorCode;
  /** Makes the next `processImage` reject with this code and start nothing. */
  failNext?: OcrErrorCode;
}

/** A backend the test drives: it finishes recognitions and counts what was asked. */
export interface MemoryOcr extends OcrBackend {
  /** Answers the waiting recognition: its text, or a code to reject with. */
  finishRecognition(answer: RecognizedText | OcrErrorCode): boolean;
  pending(): boolean;
  started(): number;
  cancels(): number;
  lastImage(): ImageInput | undefined;
  lastOptions(): RecognitionOptions | undefined;
}

/** A memory backend, for tests and for apps that run without a recognizer. */
export function memoryBackend(config: MemoryOcrConfig = {}): MemoryOcr {
  let failNext = config.failNext ?? null;
  // Recognition is compute, not UI: calls wait concurrently, answered in start order.
  const waiting: Array<(answer: RecognizedText | OcrErrorCode) => void> = [];
  let started = 0;
  let cancels = 0;
  let image: ImageInput | undefined;
  let options: RecognitionOptions | undefined;

  return {
    supportedLanguages: async () => {
      if (typeof config.supportedLanguages === "string") throw ocrError(config.supportedLanguages);
      return [...(config.supportedLanguages ?? ["en-US"])];
    },
    processImage: (nextImage: ImageInput, nextOptions: RecognitionOptions) => {
      image = nextImage;
      options = nextOptions;
      if (failNext !== null) {
        const code = failNext;
        failNext = null;
        return Promise.reject(ocrError(code));
      }
      started += 1;
      return new Promise<RecognizedText>((resolve, reject) => {
        waiting.push((answer) => {
          if (typeof answer === "string") reject(ocrError(answer));
          else resolve(answer);
        });
      });
    },
    cancel: async () => {
      cancels += 1;
      return waiting.length > 0;
    },
    close: async () => {
      // Nothing is held: a memory backend has no recognizer to release.
    },
    finishRecognition: (answer) => {
      const done = waiting.shift();
      done?.(answer);
      return done !== undefined;
    },
    pending: () => waiting.length > 0,
    started: () => started,
    cancels: () => cancels,
    lastImage: () => image,
    lastOptions: () => options,
  };
}
