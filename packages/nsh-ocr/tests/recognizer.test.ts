import { describe, expect, it, vi } from "vitest";
import { TextRecognizer, memoryBackend, type ImageInput, type RecognizedText } from "../src";

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

describe("processImage", () => {
  it("a call resolves once with the recognized text", async () => {
    const backend = memoryBackend();
    const recognizer = new TextRecognizer({ backend });
    const call = recognizer.processImage(image());
    await settle(() => backend.pending());
    expect(backend.started()).toBe(1);
    expect(backend.lastImage()).toEqual(image());
    expect(backend.finishRecognition(text("Hello, OCR"))).toBe(true);
    const answer = await call;
    expect(answer.text).toBe("Hello, OCR");
    expect(answer.blocks).toEqual([]);
    // A second answer has no call left to resolve.
    expect(backend.finishRecognition(text("again"))).toBe(false);
  });

  it("concurrent calls each resolve once", async () => {
    const backend = memoryBackend();
    const recognizer = new TextRecognizer({ backend });
    const first = recognizer.processImage(image());
    await settle(() => backend.pending());
    const second = recognizer.processImage(image());
    await settle(() => backend.started() === 2);
    expect(backend.pending()).toBe(true);
    // Both answers carry the same text, so whichever call is answered first is irrelevant.
    expect(backend.finishRecognition(text("both"))).toBe(true);
    expect(backend.finishRecognition(text("both"))).toBe(true);
    expect((await first).text).toBe("both");
    expect((await second).text).toBe("both");
    expect(backend.finishRecognition(text("third"))).toBe(false);
    expect(backend.started()).toBe(2);
  });
});

describe("cancelling", () => {
  it("cancel resolves pending calls with the cancelled code", async () => {
    const backend = memoryBackend();
    const recognizer = new TextRecognizer({ backend });
    const first = recognizer.processImage(image());
    await settle(() => backend.pending());
    const second = recognizer.processImage(image());
    await settle(() => backend.started() === 2);
    expect(await recognizer.cancel()).toBe(true);
    await expect(first).rejects.toMatchObject({ code: "ERR_OCR_CANCELLED" });
    await expect(second).rejects.toMatchObject({ code: "ERR_OCR_CANCELLED" });
    expect(backend.cancels()).toBeGreaterThanOrEqual(1);
  });

  it("cancel with nothing pending reports false", async () => {
    const recognizer = new TextRecognizer({ backend: memoryBackend() });
    expect(await recognizer.cancel()).toBe(false);
  });

  it("a late answer after a cancel is dropped", async () => {
    const backend = memoryBackend();
    const recognizer = new TextRecognizer({ backend });
    const call = recognizer.processImage(image());
    await settle(() => backend.pending());
    expect(await recognizer.cancel()).toBe(true);
    await expect(call).rejects.toMatchObject({ code: "ERR_OCR_CANCELLED" });
    expect(backend.cancels()).toBe(1);
    // The backend still holds the cancelled call's resolver; the recognizer drops its answer.
    expect(backend.finishRecognition(text("late"))).toBe(true);
    const next = recognizer.processImage(image());
    await settle(() => backend.started() === 2);
    expect(backend.finishRecognition(text("after"))).toBe(true);
    expect((await next).text).toBe("after");
    expect(backend.finishRecognition(text("late again"))).toBe(false);
  });

  it("dropping the call asks the backend to cancel", async () => {
    const backend = memoryBackend();
    const recognizer = new TextRecognizer({ backend });
    const call = recognizer.processImage(image());
    await settle(() => backend.pending());
    expect(backend.cancels()).toBe(0);
    // A pending call only ends early through cancel, which asks the backend to stop.
    expect(await recognizer.cancel()).toBe(true);
    expect(backend.cancels()).toBe(1);
    await expect(call).rejects.toMatchObject({ code: "ERR_OCR_CANCELLED" });
    // The ended call's late answer is dropped, and the next call is untouched by it.
    expect(backend.finishRecognition(text("late"))).toBe(true);
    const next = recognizer.processImage(image());
    await settle(() => backend.started() === 2);
    expect(backend.pending()).toBe(true);
    expect(backend.finishRecognition(text("after"))).toBe(true);
    expect((await next).text).toBe("after");
  });
});

describe("timeouts", () => {
  it("a timeout resolves with the timeout code and the late answer is dropped", async () => {
    vi.useFakeTimers();
    try {
      const backend = memoryBackend();
      const recognizer = new TextRecognizer({ backend });
      const call = recognizer.processImage(image(), { timeoutMs: 100 });
      await settle(() => backend.pending());
      await vi.advanceTimersByTimeAsync(101);
      await expect(call).rejects.toMatchObject({ code: "ERR_OCR_TIMEOUT" });
      // The late answer for the timed-out call is dropped, so the next call still
      // waits for its own answer and resolves with it.
      expect(backend.finishRecognition(text("late"))).toBe(true);
      const next = recognizer.processImage(image());
      await settle(() => backend.started() === 2);
      expect(backend.pending()).toBe(true);
      expect(backend.finishRecognition(text("after"))).toBe(true);
      expect((await next).text).toBe("after");
      expect(backend.finishRecognition(text("late again"))).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  it("a zero timeout is refused as an invalid argument", async () => {
    const backend = memoryBackend();
    const recognizer = new TextRecognizer({ backend });
    await expect(recognizer.processImage(image(), { timeoutMs: 0 })).rejects.toMatchObject({
      code: "ERR_OCR_INVALID_ARGUMENT",
    });
    expect(backend.started()).toBe(0);
  });
});

describe("closing", () => {
  it("a closed recognizer refuses processImage with the closed code", async () => {
    const backend = memoryBackend();
    const recognizer = new TextRecognizer({ backend });
    await recognizer.close();
    await expect(recognizer.processImage(image())).rejects.toMatchObject({ code: "ERR_OCR_CLOSED" });
    expect(backend.started()).toBe(0);
  });

  it("close is idempotent", async () => {
    const backend = memoryBackend();
    const recognizer = new TextRecognizer({ backend });
    const call = recognizer.processImage(image());
    await settle(() => backend.pending());
    await recognizer.close();
    await recognizer.close();
    await expect(call).rejects.toMatchObject({ code: "ERR_OCR_CANCELLED" });
    expect(backend.cancels()).toBe(1);
    await expect(recognizer.processImage(image())).rejects.toMatchObject({ code: "ERR_OCR_CLOSED" });
  });

  it("close resolves in-flight calls as cancelled", async () => {
    const backend = memoryBackend();
    const recognizer = new TextRecognizer({ backend });
    const first = recognizer.processImage(image());
    await settle(() => backend.pending());
    const second = recognizer.processImage(image());
    await settle(() => backend.started() === 2);
    await recognizer.close();
    await expect(first).rejects.toMatchObject({ code: "ERR_OCR_CANCELLED" });
    await expect(second).rejects.toMatchObject({ code: "ERR_OCR_CANCELLED" });
    expect(backend.cancels()).toBeGreaterThanOrEqual(1);
    await expect(recognizer.processImage(image())).rejects.toMatchObject({ code: "ERR_OCR_CLOSED" });
  });
});
