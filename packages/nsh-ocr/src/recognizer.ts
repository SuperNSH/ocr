import type { OcrBackend } from "./backend";
import { asOcrError, ocrError, type OcrError } from "./errors";
import { validateImageInput, type ImageInput } from "./input";
import type { RecognitionOptions, Script } from "./options";
import { rustBackend } from "./rust";
import type { RecognizedText } from "./text";

/** The arguments of `processImage`, with the recognizer's names. */
export interface RecognizeArgs {
  image: ImageInput;
  options?: RecognitionOptions;
}

type Outcome = { ok: true; value: RecognizedText } | { ok: false; error: OcrError };

/**
 * Text recognition, with the recognizers' names. It runs on the Rust crate unless it is given
 * another backend. Recognition is compute, not UI: calls run concurrently, and each resolves
 * exactly once.
 */
export class TextRecognizer {
  private readonly backend: OcrBackend;
  private readonly script: Script;
  private closed = false;
  private readonly pending = new Set<(outcome: Outcome) => void>();

  constructor(options: { backend?: OcrBackend; script?: Script } = {}) {
    this.backend = options.backend ?? rustBackend();
    this.script = options.script ?? "latin";
  }

  /** The BCP-47 languages the backend can recognise now. */
  async supportedLanguages(): Promise<string[]> {
    try {
      return await this.backend.supportedLanguages();
    } catch (error: unknown) {
      throw asOcrError(error);
    }
  }

  /**
   * Recognises text in `image`. An image with no text resolves with empty `text` and no blocks;
   * every failure rejects with an `OcrError` that has its code. A closed recognizer refuses with
   * `ERR_OCR_CLOSED`, and a zero `timeoutMs` is `ERR_OCR_INVALID_ARGUMENT`. With `timeoutMs` set,
   * the call gives up at the deadline with `ERR_OCR_TIMEOUT` and the late answer is dropped.
   */
  processImage(image: ImageInput, options: RecognitionOptions = {}): Promise<RecognizedText> {
    try {
      validateImageInput(image);
      if (options.timeoutMs !== undefined && !(options.timeoutMs > 0)) {
        throw ocrError("ERR_OCR_INVALID_ARGUMENT");
      }
      if (this.closed) throw ocrError("ERR_OCR_CLOSED");
    } catch (error: unknown) {
      return Promise.reject(error);
    }
    return new Promise<RecognizedText>((resolve, reject) => {
      let settled = false;
      let timer: ReturnType<typeof setTimeout> | null = null;
      const settle = (outcome: Outcome) => {
        if (settled) return;
        settled = true;
        if (timer !== null) clearTimeout(timer);
        this.pending.delete(settle);
        if (outcome.ok) resolve(outcome.value);
        else reject(outcome.error);
      };
      this.pending.add(settle);
      if (options.timeoutMs !== undefined) {
        timer = setTimeout(() => {
          // The race is local, like the core's sleep: no backend stop, so sibling calls are untouched.
          settle({ ok: false, error: ocrError("ERR_OCR_TIMEOUT") });
        }, options.timeoutMs);
      }
      let running: Promise<RecognizedText>;
      try {
        running = this.backend.processImage(image, this.optionsFor(options));
      } catch (error: unknown) {
        // A backend may throw before returning its promise; that rejects the call like any failure.
        running = Promise.reject(error);
      }
      running.then(
        (value) => settle({ ok: true, value }),
        (error: unknown) => settle({ ok: false, error: asOcrError(error) }),
      );
    });
  }

  /**
   * Resolves every pending call with `ERR_OCR_CANCELLED` and asks the backend to stop. Resolves
   * `true` when anything was pending.
   */
  async cancel(): Promise<boolean> {
    return this.drain();
  }

  /** Cancels in-flight calls and marks the recognizer closed. Idempotent. */
  async close(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    await this.drain();
    await this.backend.close();
  }

  /** The options the backend sees: the call's script, or the recognizer's default (`latin`). */
  private optionsFor(options: RecognitionOptions): RecognitionOptions {
    const effective: RecognitionOptions = { script: options.script ?? this.script };
    if (options.timeoutMs !== undefined) effective.timeoutMs = options.timeoutMs;
    return effective;
  }

  /** Ends every pending call as cancelled, and asks the backend to stop when any ended. */
  private async drain(): Promise<boolean> {
    const settles = [...this.pending];
    this.pending.clear();
    for (const settle of settles) settle({ ok: false, error: ocrError("ERR_OCR_CANCELLED") });
    if (settles.length > 0) await this.backend.cancel();
    return settles.length > 0;
  }
}
