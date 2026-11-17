import { mount, signal, Text, View } from "nsh";
import {
  OcrError,
  TextRecognizer,
  rustBackend,
  useOcrsModels,
  type RgbaImage,
} from "../../packages/nsh-ocr/src";
// The `?url` imports ship the model files with the app and give their `asset:/` URLs.
import detectionModelUrl from "./assets/text-detection.onnx?url";
import recognitionModelUrl from "./assets/text-recognition.onnx?url";

/** The device's OCR engine, through the Rust crate. */
const recognizer = new TextRecognizer({ backend: rustBackend() });

/** What the last press showed: the recognized text, or the error's code. */
const result = signal("Not checked yet");

/** The engine's languages, once supportedLanguages answers. */
const languages = signal("Checking languages");

/** The model files' state: the Linux and web engines need them, the others' are built in. */
const models = signal("Loading models");

function codeOf(error: unknown): string {
  return error instanceof OcrError ? error.code : "unknown error";
}

// A 5x7-ish bitmap font, rendered big and high-contrast so every engine reads it.
const GLYPHS: Record<string, string[]> = {
  C: ["###", "#  ", "#  ", "#  ", "#  ", "#  ", "###"],
  E: ["###", "#  ", "#  ", "## ", "#  ", "#  ", "###"],
  H: ["#  #", "#  #", "#  #", "####", "#  #", "#  #", "#  #"],
  L: ["#  ", "#  ", "#  ", "#  ", "#  ", "#  ", "###"],
  O: ["###", "#  #", "#  #", "#  #", "#  #", "#  #", "###"],
  R: ["### ", "#  #", "#  #", "### ", "# # ", "#  #", "#  #"],
  " ": ["  ", "  ", "  ", "  ", "  ", "  ", "  "],
};

/** "HELLO OCR" as raw RGBA pixels, white on black. */
function helloImage(): RgbaImage {
  const text = "HELLO OCR";
  const scale = 12;
  const margin = 2;
  const glyphWidth = (glyph: string[]) => glyph[0].length;
  const glyphs = [...text].map((ch) => GLYPHS[ch]).filter((glyph) => glyph !== undefined);
  const width = (glyphs.reduce((sum, glyph) => sum + glyphWidth(glyph) + 1, -1) + 2 * margin) * scale;
  const height = (7 + 2 * margin) * scale;
  const data = new Uint8Array(width * height * 4);
  let x = margin * scale;
  for (const glyph of glyphs) {
    for (let gy = 0; gy < 7; gy++) {
      const row = glyph[gy];
      for (let gx = 0; gx < row.length; gx++) {
        if (row[gx] !== "#") continue;
        for (let sy = 0; sy < scale; sy++) {
          for (let sx = 0; sx < scale; sx++) {
            const i = (((margin + gy) * scale + sy) * width + x + gx * scale + sx) * 4;
            data[i] = 255;
            data[i + 1] = 255;
            data[i + 2] = 255;
            data[i + 3] = 255;
          }
        }
      }
    }
    x += (glyphWidth(glyph) + 1) * scale;
  }
  return { width, height, data };
}

function recognize() {
  result.set("Recognizing");
  recognizer
    .processImage(helloImage(), { script: "latin" })
    .then(
      (text) => result.set(text.text || "(no text found)"),
      (error: unknown) => result.set(codeOf(error)),
    );
}

recognizer
  .supportedLanguages()
  .then(
    (langs) => languages.set(langs.join(", ") || "(none)"),
    (error: unknown) => languages.set(codeOf(error)),
  );

async function loadModels(): Promise<void> {
  try {
    const [detection, recognition] = await Promise.all([
      fetch(detectionModelUrl).then((response) => response.arrayBuffer()),
      fetch(recognitionModelUrl).then((response) => response.arrayBuffer()),
    ]);
    await useOcrsModels(new Uint8Array(detection), new Uint8Array(recognition));
    models.set("Models loaded");
  } catch (error: unknown) {
    // The Apple, Android and Windows engines are built in; they need no model files.
    if (error instanceof OcrError && error.code === "ERR_OCR_NOT_AVAILABLE") {
      models.set("Models not needed");
      return;
    }
    models.set(codeOf(error));
  }
}

loadModels();

function App() {
  return (
    <View class="flex-1 gap-4 p-4">
      <Text testID="languages" class="text-foreground">{languages()}</Text>
      <Text testID="models" class="text-foreground">{models()}</Text>
      <Text testID="result" class="text-foreground">{result()}</Text>
      <Text testID="recognize" role="button" onPress={recognize} class="rounded-(--radius) bg-muted px-3 py-2">
        Recognize
      </Text>
    </View>
  );
}

mount(App);
