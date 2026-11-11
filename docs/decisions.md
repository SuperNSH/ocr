# Decisions

Each entry says what was chosen and why. `docs/parity.md` lists what each API does on each platform.

## References cloned into `.refs/` (2026-11-09)

Read before any code was written. `.refs/` is git-ignored and never built or run.

1. `https://github.com/flutter-ml/google_ml_kit_flutter` — the Flutter reference. `packages/google_mlkit_text_recognition/lib/src/text_recognizer.dart`: `TextRecognizer({script})`, `processImage(InputImage) -> RecognizedText {text, blocks}`, `close()`, `TextRecognitionScript {latin, chinese, devanagiri, japanese, korean}`, and the `TextBlock`/`TextLine`/`TextElement`/`TextSymbol` tree with `boundingBox`, `recognizedLanguages`, `cornerPoints`, and Android-only `confidence`/`angle`.
2. `https://github.com/robertknight/ocrs` — the pure-Rust engine. `ocrs 0.14.0`: `OcrEngine::new(OcrEngineParams {detection_model, recognition_model, ...})`, `prepare_input(ImageSource)`, `get_text() -> Result<String, OcrError>`, `OcrErrorKind {DetectionModelNotLoaded, RecognitionModelNotLoaded, IncompatibleModel, InferenceFailed}`, results as `TextLine`/`TextWord`/`TextItem`/`TextChar`. Models come as files the app ships; the engine takes raw pixels, not paths.
3. `https://github.com/longbridge/nsh` (NSH main) — the crates we wrap. `nsh-core` 0.4.0 gives `#[class]`, `#[derive(JsObject)]`, `#[native_module]`, `JsError.with_code`; `runtime/fs.rs` shows binary data crossing the TS↔Rust bridge; `runtime/image.rs` shows how NSH names image values.
4. `https://github.com/SuperNSH/biometrics` — the org layout this repo copies: `crates/nsh-<name>` (Rust: `error.rs` coded errors, `backend.rs` trait, `fake.rs` scripted mock, `native.rs` TSX class, `platform/` per-target backends) and `packages/nsh-<name>` (TypeScript mirror with `rustBackend()`, `memoryBackend()`, the same error codes), with CI running format, a three-OS Rust matrix, cross-target checks and `nsh check/build/test`.
5. The React Native reference, found and verified — see the next entry.

## The React Native reference is `infinitered/react-native-mlkit`, module `react-native-mlkit-text-recognition`

Chosen on 2026-11-09. The module is `@infinitered/react-native-mlkit-text-recognition` (MIT on npm, repo at v6.0.0 with npm latest 5.0.1 published 2025-11-25). Why this one:

- **Maintained.** The repo was pushed on 2026-10-29 and merged an Expo SDK 56 / RN 0.85 upgrade in 2026-07; it is an Expo module with CI, changesets and a monorepo of sibling ML Kit modules. Owner: Infinite Red, an established React Native consultancy.
- **Right API shape.** `recognizeText(imagePath) -> Text {text, blocks}` with `Block`/`TextLine`/`TextElement`, each carrying `text`, `frame {left, top, right, bottom}` and `recognizedLanguages` — the same ML Kit shape as the Flutter reference, which is what `docs/parity.md` maps.

Rejected: `agoldis/react-native-mlkit-ocr` — archived by its owner on 2025-12-09; `react-native-vision-camera-text-recognition` — a VisionCamera frame-processor plugin, not a standalone recognition API. No given URL failed; the brief did not name an RN library, so one was found and verified as it asks.

## The engines, per platform

`docs/brief.md` recommends Vision on Apple and ocrs elsewhere; the bridges follow it.

- **Apple (macOS, iOS)**: Vision's `VNRecognizeTextRequest`, with the request's `supportedRecognitionLanguages` queried at runtime; a script whose language the OS does not list is `ERR_OCR_UNSUPPORTED_LANGUAGE`. Vision gives per-text confidence and no angle.
- **Android**: ML Kit text recognition v2 over JNI (the same engine the Flutter and RN references wrap), with one recognizer per script. Confidence and angle come from the platform on lines and elements.
- **Windows**: `Windows.Media.Ocr`, with `AvailableRecognizerLanguages` queried at runtime. No per-block language, no confidence, no angle.
- **Linux and web (wasm32)**: `ocrs 0.14.0`, pinned — its pre-1.0 releases break APIs, and `docs/brief.md` says to pin. Latin only, because that is what the trained models read; the model files come from the app, never from this crate. This resolves the scaffold's open platform question: iOS, Android, macOS, Windows, Linux and web (wasm32) are all in scope; every other target reports `ERR_OCR_NOT_AVAILABLE`.

These three entries resolve the scaffold's open decisions 1–3 (NSH version, library choice, platforms in scope).

## NSH is pinned to 0.4.0

`nsh-core = "0.4.0"` from crates.io, like biometrics; CI installs the NSH CLI at `NSH_VERSION: v0.4.0`. `longbridge/nsh` `main` is ahead of it, so anything read in `.refs/nsh` is checked against the 0.4.0 tag before it is relied on.

## The input is encoded bytes or raw RGBA

`ImageInput` is `Encoded(Vec<u8>)` (a PNG/JPEG file's bytes) or `Rgba {width, height, data}` (row-major RGBA8). This is the common denominator: Vision takes both through CoreGraphics, ML Kit takes both through Android decoders, and ocrs takes pixels directly — so the crate needs no image-decoding dependency on Apple, Android or Windows, and the one on Linux/web stays inside the ocrs backend. The references' path-based APIs (`InputImage.fromFilePath`, `recognizeText(imagePath)`) map to the app handing over bytes it read, plus a `path` field on the native TSX class for the common case.

## Nothing is built on this machine

`AGENTS.md` forbids building, testing or running emulators on a developer machine, and CI compiles and tests everything. The one local check is `rustfmt --check`, which only parses and formats. `cargo clippy`, `cargo test`, `cargo check` and the TypeScript checks run in GitHub Actions (`.github/workflows/ci.yml`); until a push runs CI, no status claim in this repo is verified.

## The demo APK is a Gradle shell around NSH's APK

NSH 0.4.0 writes the Android APK itself and offers no way to add Java code or
libraries, but the Android bridge needs both: the `dev.supernsh.ocr.NativeCallback`
class (source in `crates/nsh-ocr/src/platform/android.rs`) and ML Kit's
artifacts, which carry Android resources a hand merge cannot combine. So CI
builds a thin Gradle app (`examples/demo/android/`) that carries ML Kit and the
callback class, then `merge-nsh-into-apk.sh` zips NSH's `classes.dex` (as
`classes2.dex`), `lib/` and `assets/` into it, re-aligns and re-signs. The
manifest replicates NSH's own (`nsh-cli/src/android.rs` `manifest_xml`) so the
same `com.longbridge.nsh.AppActivity` starts. Only the Latin model ships in the
demo APK (`com.google.mlkit:text-recognition` 16.0.1); the other scripts'
recognizers are not bundled and answer a coded error on the device.
