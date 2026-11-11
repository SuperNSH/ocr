import { expect, test } from "nsh/test";
import {
  SCRIPT_LANGUAGES,
  TextRecognizer,
  memoryBackend,
  type ImageInput,
  type RecognizedText,
} from "../src";

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

test("the result tree mirrors the reference shapes", async () => {
  const backend = memoryBackend();
  const recognizer = new TextRecognizer({ backend });
  const call = recognizer.processImage(image());
  await settle(() => backend.pending());
  expect(backend.finishRecognition(referenceTree())).toBe(true);
  const got = await call;
  expect(got).toEqual(referenceTree());
  // Spot checks down the tree: confidence and angle ride only where a platform gives them,
  // and corner points are their own data — here the word corners, not the line box's.
  const block = got.blocks[0];
  expect(block.boundingBox).toEqual({ left: 4, top: 8, right: 96, bottom: 20 });
  expect(block.cornerPoints).toEqual([
    { x: 4, y: 8 },
    { x: 40, y: 8 },
    { x: 40, y: 20 },
    { x: 4, y: 20 },
  ]);
  const line = block.lines[0];
  expect(line.confidence).toBe(0.9);
  expect(line.angle).toBe(-0.25);
  const element = line.elements[0];
  expect(element.confidence).toBe(0.95);
  expect(element.angle).toBeNull();
  const symbol = element.symbols[0];
  expect(symbol.confidence).toBe(0.91);
  expect(symbol.angle).toBe(0.5);
});

test("an image with no text resolves with empty text and no blocks", async () => {
  const backend = memoryBackend();
  const recognizer = new TextRecognizer({ backend });
  const call = recognizer.processImage(image());
  await settle(() => backend.pending());
  expect(backend.finishRecognition({ text: "", blocks: [] })).toBe(true);
  const empty = await call;
  expect(empty.text).toBe("");
  expect(empty.blocks).toEqual([]);
});

test("Unicode text passes through unchanged", async () => {
  const backend = memoryBackend();
  const recognizer = new TextRecognizer({ backend });
  const call = recognizer.processImage(image());
  await settle(() => backend.pending());
  expect(backend.finishRecognition(unicodeTree())).toBe(true);
  const got = await call;
  expect(got).toEqual(unicodeTree());
  expect(got.text).toBe("こんにちは、世界 — café naïve Ω 📷");
  expect(got.blocks[0].lines[0].elements[0].symbols[0].text).toBe("こんにちは、世界");
});

test("each script has the name TSX uses", () => {
  expect(SCRIPT_LANGUAGES).toEqual({
    latin: "en-US",
    chinese: "zh-Hans",
    devanagari: "hi-IN",
    japanese: "ja-JP",
    korean: "ko-KR",
  });
});

test("the script and options reach the backend", async () => {
  const backend = memoryBackend();
  const recognizer = new TextRecognizer({ backend });
  const call = recognizer.processImage(image(), { script: "japanese", timeoutMs: 60000 });
  await settle(() => backend.pending());
  expect(backend.lastImage()).toEqual(image());
  expect(backend.lastOptions()?.script).toBe("japanese");
  expect(backend.lastOptions()?.timeoutMs).toBe(60000);
  expect(backend.finishRecognition(text("日本語"))).toBe(true);
  expect((await call).text).toBe("日本語");
});

test("supported languages pass through the backend", async () => {
  const ready = memoryBackend({ supportedLanguages: ["ja-JP", "ko-KR"] });
  const working = new TextRecognizer({ backend: ready });
  expect(await working.supportedLanguages()).toEqual(["ja-JP", "ko-KR"]);

  const broken = memoryBackend({ supportedLanguages: "ERR_OCR_NOT_AVAILABLE" });
  const failing = new TextRecognizer({ backend: broken });
  await expect(failing.supportedLanguages()).rejects.toMatchObject({
    code: "ERR_OCR_NOT_AVAILABLE",
  });
});

/** The reference shape: every level present, confidence and angle set only where a platform gives them. */
function referenceTree(): RecognizedText {
  const wordBox = { left: 4, top: 8, right: 40, bottom: 20 };
  const lineBox = { left: 4, top: 8, right: 96, bottom: 20 };
  const wordCorners = [
    { x: 4, y: 8 },
    { x: 40, y: 8 },
    { x: 40, y: 20 },
    { x: 4, y: 20 },
  ];
  return {
    text: "Hello, OCR",
    blocks: [
      {
        text: "Hello, OCR",
        lines: [
          {
            text: "Hello, OCR",
            elements: [
              {
                text: "Hello,",
                symbols: [
                  {
                    text: "Hello,",
                    boundingBox: wordBox,
                    languages: ["en-US"],
                    cornerPoints: wordCorners,
                    confidence: 0.91,
                    angle: 0.5,
                  },
                ],
                boundingBox: wordBox,
                languages: ["en-US"],
                cornerPoints: wordCorners,
                confidence: 0.95,
                angle: null,
              },
            ],
            boundingBox: lineBox,
            languages: ["en-US"],
            cornerPoints: wordCorners,
            confidence: 0.9,
            angle: -0.25,
          },
        ],
        boundingBox: lineBox,
        languages: ["en-US"],
        cornerPoints: wordCorners,
      },
    ],
  };
}

/** One block, one line, one element, one symbol, all carrying the same Unicode text. */
function unicodeTree(): RecognizedText {
  const label = "こんにちは、世界";
  const box = { left: 0, top: 0, right: 42, bottom: 12 };
  const symbol = {
    text: label,
    boundingBox: box,
    languages: ["ja-JP"],
    cornerPoints: [],
    confidence: null,
    angle: null,
  };
  return {
    text: `${label} — café naïve Ω 📷`,
    blocks: [
      {
        text: label,
        lines: [
          {
            text: label,
            elements: [
              {
                text: label,
                symbols: [symbol],
                boundingBox: box,
                languages: ["ja-JP"],
                cornerPoints: [],
                confidence: null,
                angle: null,
              },
            ],
            boundingBox: box,
            languages: ["ja-JP"],
            cornerPoints: [],
            confidence: null,
            angle: null,
          },
        ],
        boundingBox: box,
        languages: ["ja-JP"],
        cornerPoints: [],
      },
    ],
  };
}
