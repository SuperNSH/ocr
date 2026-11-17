// The demo's screen: the engine's languages, the model files' state, and the
// HELLO OCR bitmap's recognition. The engine is the host's — ocrs with the
// bundled models on a Linux runner (seconds of parse and inference in a debug
// build), Vision on macOS — so each test polls past a step's own wait and
// accepts every honest outcome the app can show.
import { expect, open, test } from "nsh/test";

type App = Awaited<ReturnType<typeof open>>;

/** Polls the screen's `index`-th text until it leaves `initial`, and returns it. */
async function lineSettles(app: App, index: number, initial: string): Promise<string> {
  let text = initial;
  for (let tries = 0; tries < 250 && text === initial; tries++) {
    await new Promise((done) => setTimeout(done, 200));
    text = app.texts()[index] ?? initial;
  }
  return text;
}

test("the languages answer, and the model files settle", async () => {
  // The texts in order: languages, models, result, the Recognize button.
  const app = await open("../App.tsx", { width: 480, height: 800 });
  const languages = await lineSettles(app, 0, "Checking languages");
  expect(languages).not.toBe("Checking languages");
  // ocrs loads the bundled models; the built-in engines answer "not needed".
  const models = await lineSettles(app, 1, "Loading models");
  expect(["Models loaded", "Models not needed"]).toContain(models);
}, 180000);

test("Recognize settles the result line", async () => {
  const app = await open("../App.tsx", { width: 480, height: 800 });
  // ocrs refuses recognition until the models are in, so wait them in first.
  const models = await lineSettles(app, 1, "Loading models");
  expect(["Models loaded", "Models not needed"]).toContain(models);
  await app.press("id:recognize");
  // Recognized text, "(no text found)" or an ERR_OCR_* code: all honest.
  const result = await lineSettles(app, 2, "Recognizing");
  expect(result).not.toBe("Recognizing");
}, 180000);
