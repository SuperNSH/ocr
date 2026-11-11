import { describe, expect, it } from "vitest";
import { TextRecognizer, memoryBackend, type ImageInput, type RecognizedText } from "../src";
import { MESSAGES, type OcrErrorCode } from "../src/errors";

/** Any non-empty payload: validation passes, and the memory backend decodes nothing. */
function image(): ImageInput {
  return { encoded: new Uint8Array([1, 2, 3, 4]) };
}

/** A minimal successful answer carrying `label` as the joined text. */
function text(label: string): RecognizedText {
  return { text: label, blocks: [] };
}

/** Yields microtasks until `ready` holds, so a call has reached the backend. */
async function settle(ready: () => boolean): Promise<void> {
  for (let turn = 0; turn < 50 && !ready(); turn += 1) await Promise.resolve();
  if (!ready()) throw new Error("the call never reached the backend");
}

const CODES: OcrErrorCode[] = [
  "ERR_OCR_NOT_AVAILABLE",
  "ERR_OCR_UNSUPPORTED_LANGUAGE",
  "ERR_OCR_MODEL_NOT_LOADED",
  "ERR_OCR_INVALID_IMAGE",
  "ERR_OCR_INVALID_ARGUMENT",
  "ERR_OCR_IO_FAILED",
  "ERR_OCR_CLOSED",
  "ERR_OCR_CANCELLED",
  "ERR_OCR_TIMEOUT",
  "ERR_OCR_BACKEND",
];

describe("image validation", () => {
  it("an RGBA size mismatch is refused as an invalid image", async () => {
    const backend = memoryBackend();
    const recognizer = new TextRecognizer({ backend });
    // 2x2 RGBA needs 16 bytes; 15 is a torn read.
    const torn = { width: 2, height: 2, data: new Uint8Array(15) };
    await expect(recognizer.processImage(torn)).rejects.toMatchObject({
      code: "ERR_OCR_INVALID_IMAGE",
    });
    expect(backend.started()).toBe(0);
  });

  it("zero dimensions are refused as an invalid image", async () => {
    const backend = memoryBackend();
    const recognizer = new TextRecognizer({ backend });
    const zeroWidth = { width: 0, height: 2, data: new Uint8Array(0) };
    const zeroHeight = { width: 2, height: 0, data: new Uint8Array(0) };
    for (const torn of [zeroWidth, zeroHeight]) {
      await expect(recognizer.processImage(torn)).rejects.toMatchObject({
        code: "ERR_OCR_INVALID_IMAGE",
      });
    }
    expect(backend.started()).toBe(0);
  });

  it("empty encoded bytes are refused as an invalid image", async () => {
    const backend = memoryBackend();
    const recognizer = new TextRecognizer({ backend });
    await expect(recognizer.processImage({ encoded: new Uint8Array(0) })).rejects.toMatchObject({
      code: "ERR_OCR_INVALID_IMAGE",
    });
    expect(backend.started()).toBe(0);
    // Non-empty bytes pass validation and reach the backend.
    const call = recognizer.processImage(image());
    await settle(() => backend.pending());
    expect(backend.finishRecognition(text("decoded"))).toBe(true);
    expect((await call).text).toBe("decoded");
  });
});

describe("error kinds and codes", () => {
  it("every kind reaches the caller with its code", async () => {
    const backend = memoryBackend();
    const recognizer = new TextRecognizer({ backend });
    for (const code of CODES) {
      const call = recognizer.processImage(image());
      await settle(() => backend.pending());
      expect(backend.finishRecognition(code)).toBe(true);
      await expect(call).rejects.toMatchObject({ code });
    }
  });

  it("every kind has a distinct code", () => {
    expect(new Set(CODES).size).toBe(CODES.length);
    for (const code of CODES) {
      expect(MESSAGES[code].length).toBeGreaterThan(0);
    }
  });

  it("a platform status is kept with its error", async () => {
    const backend = memoryBackend();
    const recognizer = new TextRecognizer({ backend });
    const call = recognizer.processImage(image());
    await settle(() => backend.pending());
    expect(backend.finishRecognition("ERR_OCR_BACKEND")).toBe(true);
    // The status itself stays on the Rust side; the code and its frozen wording travel together.
    await expect(call).rejects.toMatchObject({
      code: "ERR_OCR_BACKEND",
      message: MESSAGES["ERR_OCR_BACKEND"],
    });
  });
});

describe("backend failures", () => {
  it("an unsupported script resolves with the unsupported language code", async () => {
    const backend = memoryBackend({ failNext: "ERR_OCR_UNSUPPORTED_LANGUAGE" });
    const recognizer = new TextRecognizer({ backend });
    await expect(recognizer.processImage(image(), { script: "korean" })).rejects.toMatchObject({
      code: "ERR_OCR_UNSUPPORTED_LANGUAGE",
    });
    expect(backend.started()).toBe(0);
    const call = recognizer.processImage(image());
    await settle(() => backend.pending());
    expect(backend.finishRecognition(text("runs"))).toBe(true);
    expect((await call).text).toBe("runs");
  });

  it("a backend engine failure resolves with the backend code", async () => {
    const backend = memoryBackend();
    const recognizer = new TextRecognizer({ backend });
    const call = recognizer.processImage(image());
    await settle(() => backend.pending());
    expect(backend.finishRecognition("ERR_OCR_BACKEND")).toBe(true);
    await expect(call).rejects.toMatchObject({ code: "ERR_OCR_BACKEND" });
  });

  it("a model error resolves with the model code", async () => {
    const backend = memoryBackend({ failNext: "ERR_OCR_MODEL_NOT_LOADED" });
    const recognizer = new TextRecognizer({ backend });
    await expect(recognizer.processImage(image())).rejects.toMatchObject({
      code: "ERR_OCR_MODEL_NOT_LOADED",
    });
    expect(backend.started()).toBe(0);
    const call = recognizer.processImage(image());
    await settle(() => backend.pending());
    expect(backend.finishRecognition(text("runs"))).toBe(true);
    expect((await call).text).toBe("runs");
  });
});
