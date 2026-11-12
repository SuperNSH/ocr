# Goals

`nsh-ocr` recognises text in images for Native Shell apps. It is a Rust crate, `crates/nsh-ocr` (lib `nsh_ocr`), and a TypeScript package, `packages/nsh-ocr`, laid out like `SuperNSH/biometrics`: the same names and the same behaviour on both sides, and every behaviour tested in both languages. Recognition runs on the platform's engine — Vision on Apple, ML Kit text recognition v2 on Android, `Windows.Media.Ocr` on Windows, `ocrs` on Linux and web (wasm32) — and every other target answers `ERR_OCR_NOT_AVAILABLE`.

It follows two API references and four engines (the full catalogue is in `docs/decisions.md`):

- **Flutter `google_mlkit_text_recognition`** — the API shape: a `TextRecognizer` built for one script, `processImage` returning the `RecognizedText` → `TextBlock` → `TextLine` → `TextElement` → `TextSymbol` tree with bounding box, recognized languages and corner points, then `close()`.
- **`infinitered/react-native-mlkit-text-recognition`** — the path-based call, `recognizeText(imagePath)`, and the ML Kit tree as TypeScript sees it: `Text` → `Block` → `TextLine` → `TextElement` with `frame` and `recognizedLanguages` only — no symbols, corner points, confidence or angle.
- **ocrs** (Linux, web), **Vision** (Apple), **ML Kit** (Android), **Windows.Media.Ocr** (Windows) — the engines; Vision and Windows are queried at runtime for what they support, while ML Kit and ocrs answer from their fixed script sets.

## Acceptance criteria

The gates, all in `.github/workflows/ci.yml`:

- `cargo fmt --all -- --check` (ubuntu-latest).
- `cargo clippy -p nsh-ocr --all-targets --all-features -- -D warnings`, then `cargo test -p nsh-ocr --all-features --no-fail-fast`, on ubuntu-latest, macos-latest and windows-latest, fail-fast off.
- Cross-target clippy — `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `aarch64-linux-android` (biometrics' NDK env block) and `wasm32-unknown-unknown` — with `cargo clippy -p nsh-ocr --target <target> --all-features -- -D warnings`.
- TypeScript in `packages/nsh-ocr` under NSH v0.5.0: `nsh check`; `nsh build` with no warning in its log; `nsh test`.

The rules behind the gates:

- **Resolve exactly once.** Every operation ends in success, a coded error, or a cancel. It never hangs, and a late platform answer that arrives after the call resolved is dropped.
- **No panics in any bridge.** No JNI callback, Vision handler, Windows async join or ocrs call may panic; every failure becomes a coded error.
- **Parity.** Every public API of the two references has a row in `docs/parity.md`.
- **Layout.** `crates/nsh-ocr` plus `packages/nsh-ocr`, like biometrics: same names, same behaviour, same error wording in Rust and TSX.

Every gate runs in GitHub Actions. Nothing in this repo is verified until a run on `master` passes; until then every status claim, including everything below, is unverified (AGENTS.md).

## Errors

Ten kinds, each with a stable code and a frozen message, identical in the Rust crate (`ErrorKind`, `src/error.rs`) and the TS package (`OcrErrorCode`, `src/errors.ts`). `OcrError` can carry a platform status (`with_status`) next to the kind; the kind is what callers switch on.

| Kind | Code | Message | When it happens | Reference failure it covers |
|---|---|---|---|---|
| NotAvailable | `ERR_OCR_NOT_AVAILABLE` | Text recognition is not available on this device. | Other targets: every call, from the unsupported backend. Never on Apple, Android, Windows, Linux or web — each ships an engine. | None of the references runs outside its platforms; this is our catch-all. |
| UnsupportedLanguage | `ERR_OCR_UNSUPPORTED_LANGUAGE` | This script is not supported by this recognizer. | Apple: the script's primary language is missing from the request's runtime-queried `supportedRecognitionLanguages`, checked before starting. Windows: `TryCreateFromLanguage` fails — no installed recognizer language matches. Linux/web: any script other than Latin. Android: never — each of the five scripts has its own bundled recognizer. | Vision's unsupported recognition language. |
| ModelNotLoaded | `ERR_OCR_MODEL_NOT_LOADED` | The recognition model files were not provided or are incompatible. | Linux/web only: `use_ocrs_models` was never called, or the detection or recognition bytes fail to parse. Never on Apple, Android or Windows — the OS or the app bundle ships the model. | ocrs' `DetectionModelNotLoaded`, `RecognitionModelNotLoaded` and `IncompatibleModel`. |
| InvalidImage | `ERR_OCR_INVALID_IMAGE` | The image is empty, unreadable, or has the wrong shape. | Every platform, before any work: `Encoded` bytes empty, or `Rgba` with a zero dimension or `data.len() != width * height * 4`. Then per platform: Apple — bytes `CGImageSource` cannot decode; Android — bytes `BitmapFactory` cannot decode into an `InputImage`; Windows — bytes `BitmapDecoder` cannot decode; Linux/web — bytes the `image` crate cannot decode. | ML Kit's undecodable `InputImage`; the bad bytes behind a bad `imagePath`. |
| InvalidArgument | `ERR_OCR_INVALID_ARGUMENT` | An argument is empty or not allowed on this platform. | Every platform, before any work: `timeout_ms` is `Some(0)`; on TSX, a script name outside the five. | The Flutter plugin's `invalid_args`. |
| IoFailed | `ERR_OCR_IO_FAILED` | The image file could not be read. | Every platform, in the native TSX layer only: reading the `path` handed to `processImage` fails. The Rust core never raises it — it takes bytes, not paths. | The RN module's `recognizeText(imagePath)` on an unreadable path. |
| Closed | `ERR_OCR_CLOSED` | This recognizer is closed. | Every platform, in the core: `processImage` after `close()`. | ML Kit's closed-recognizer state. |
| Cancelled | `ERR_OCR_CANCELLED` | The recognition was cancelled. | Every platform, in the core: `cancel()` with a call pending, or `close()` with a call in flight. No platform reports it itself. | The user- or app-cancelled scan. |
| Timeout | `ERR_OCR_TIMEOUT` | The recognition timed out. Try again. | Every platform, in the core: `timeoutMs` elapses before the answer — the Rust timer on non-wasm32, the TypeScript watchdog on wasm32 — and the late answer is dropped. | None; no reference has a timeout. |
| Backend | `ERR_OCR_BACKEND` | The platform's recognition engine failed. | Apple: `VNRecognizeTextRequest` fails for a reason other than unsupported language. Android: the ML Kit `Task` fails. Windows: `RecognizeAsync` fails. Linux/web: ocrs' `InferenceFailed`. | ML Kit's failed `Task` (the plugin's `TextRecognizerError`); ocrs' `InferenceFailed`. |

## Edge cases

The matrix below is frozen: these rows, these names, these files, in both languages. Do not rename a test, do not add a row; a case found missing goes under "Known gaps" as untested, never as a silent contract change. Rust tests live in `crates/nsh-ocr/tests/`, TypeScript tests in `packages/nsh-ocr/tests/`, and both drive only `FakeBackend` / `memoryBackend`.

| # | Tests | Rust fn | TypeScript `it()` | What the references do there |
|---|---|---|---|---|
| 1 | `recognizer.rs` / `recognizer.test.ts` | `a_call_resolves_once_with_the_recognized_text` | a call resolves once with the recognized text | Flutter's `processImage` resolves once with the whole `RecognizedText`; ours resolves exactly once even when a cancel, timeout or drop races the answer. |
| 2 | `recognizer.rs` / `recognizer.test.ts` | `concurrent_calls_each_resolve_once` | concurrent calls each resolve once | Flutter keeps one recognizer instance, the RN module builds a fresh client per call, and neither gates overlapping calls; recognition is compute, not a prompt, so each call gets its own answer. |
| 3 | `recognizer.rs` / `recognizer.test.ts` | `cancel_resolves_pending_calls_with_the_cancelled_code` | cancel resolves pending calls with the cancelled code | No reference has a cancel API; where their platforms stop on an abandoned scan, we resolve every pending call with `ERR_OCR_CANCELLED`. |
| 4 | `recognizer.rs` / `recognizer.test.ts` | `cancel_with_no_pending_call_reports_false` | cancel with nothing pending reports false | With nothing running there is nothing to stop, so cancel reports false instead of inventing work. |
| 5 | `recognizer.rs` / `recognizer.test.ts` | `a_late_answer_after_a_cancel_is_dropped` | a late answer after a cancel is dropped | Vision cannot stop running work, so a late answer can arrive after a cancel; the core drops it to keep the resolve-once rule. |
| 6 | `recognizer.rs` / `recognizer.test.ts` | `a_timeout_resolves_with_the_timeout_code_and_the_late_answer_is_dropped` | a timeout resolves with the timeout code and the late answer is dropped | `timeoutMs` is ours — no reference has one — and the timeout wins over the late platform answer, which is dropped as in row 5. |
| 7 | `recognizer.rs` / `recognizer.test.ts` | `zero_timeout_is_refused_as_an_invalid_argument` | a zero timeout is refused as an invalid argument | No reference has a timeout option; a zero is refused as a bad argument before any work starts, like any other empty or disallowed value. |
| 8 | `recognizer.rs` / `recognizer.test.ts` | `dropping_the_call_cancels_the_backend` | dropping the call asks the backend to cancel | The references' futures do not stop platform work when the caller goes away; ours deregisters and asks the backend to stop. |
| 9 | `recognizer.rs` / `recognizer.test.ts` | `a_closed_recognizer_refuses_process_image_with_the_closed_code` | a closed recognizer refuses processImage with the closed code | Neither reference shows this — the Flutter plugin silently re-creates its recognizer after `close()`, and the RN module has no `close()` — so we follow ML Kit's own closed-recognizer state and refuse with `ERR_OCR_CLOSED` before touching the backend. |
| 10 | `recognizer.rs` / `recognizer.test.ts` | `close_is_idempotent` | close is idempotent | Flutter's `close()` is fire-and-forget and harmless to repeat; ours is idempotent by contract. |
| 11 | `recognizer.rs` / `recognizer.test.ts` | `close_resolves_in_flight_calls_as_cancelled` | close resolves in-flight calls as cancelled | The references' `close()` leaves in-flight callbacks dangling; ours resolves them as cancelled so nothing hangs. |
| 12 | `errors.rs` / `errors.test.ts` | `an_rgba_size_mismatch_is_refused_as_an_invalid_image` | an RGBA size mismatch is refused as an invalid image | The references hand raw data to the platform and get a decoder error there; we refuse a length mismatch in the core, identically on every platform. |
| 13 | `errors.rs` / `errors.test.ts` | `zero_dimensions_are_refused_as_an_invalid_image` | zero dimensions are refused as an invalid image | A zero-sized image would reach the references' platform decoders; we refuse it in the core before any work starts. |
| 14 | `errors.rs` / `errors.test.ts` | `empty_encoded_bytes_are_refused_as_an_invalid_image` | empty encoded bytes are refused as an invalid image | ML Kit cannot build an `InputImage` from empty bytes; we refuse empty `Encoded` bytes at validation, before the backend. |
| 15 | `errors.rs` / `errors.test.ts` | `every_kind_reaches_the_caller_with_its_code` | every kind reaches the caller with its code | The references surface platform strings (FlutterError `TextRecognizerError`, NSError); every one of our kinds reaches the caller with its stable code. |
| 16 | `errors.rs` / `errors.test.ts` | `every_kind_has_a_distinct_code` | every kind has a distinct code | TSX switches on the code string, so all ten codes are distinct by construction; this test pins it. |
| 17 | `errors.rs` / `errors.test.ts` | `a_platform_status_is_kept_with_its_error` | a platform status is kept with its error | Vision, ML Kit and Windows each attach a status to failures (NSError code, Task error, HRESULT); `with_status` keeps it beside the kind. |
| 18 | `errors.rs` / `errors.test.ts` | `an_unsupported_script_resolves_with_the_unsupported_language_code` | an unsupported script resolves with the unsupported language code | Vision's `supportedRecognitionLanguages` missing a language, Windows' missing language pack and ocrs' Latin-only models all collapse to this one code. |
| 19 | `errors.rs` / `errors.test.ts` | `a_backend_engine_failure_resolves_with_the_backend_code` | a backend engine failure resolves with the backend code | ML Kit's failed `Task`, Vision's failed request and ocrs' `InferenceFailed` all collapse to `ERR_OCR_BACKEND`. |
| 20 | `errors.rs` / `errors.test.ts` | `a_model_error_resolves_with_the_model_code` | a model error resolves with the model code | ocrs' `DetectionModelNotLoaded`, `RecognitionModelNotLoaded` and `IncompatibleModel` all collapse to `ERR_OCR_MODEL_NOT_LOADED`. |
| 21 | `results.rs` / `results.test.ts` | `the_result_tree_mirrors_the_reference_shapes` | the result tree mirrors the reference shapes | The Flutter reference returns the full ML Kit tree — block, line, element, symbol with box, languages and corner points, `confidence` and `angle` on Android alone — while the RN module stops at element with `frame` and `recognizedLanguages` only; ours mirrors the Flutter shape. |
| 22 | `results.rs` / `results.test.ts` | `each_script_has_the_name_tsx_uses` | each script has the name TSX uses | The five scripts are Flutter's `TextRecognitionScript` with the standard `devanagari` spelling, and TSX consumes the same five strings. |
| 23 | `results.rs` / `results.test.ts` | `the_script_and_options_reach_the_backend` | the script and options reach the backend | Flutter passes `script.index` to the platform; we pass the whole options to the backend, and the fake records what reached it. |
| 24 | `results.rs` / `results.test.ts` | `supported_languages_pass_through_the_backend` | supported languages pass through the backend | The references expose no supported-languages query — Vision's `supportedRecognitionLanguages` is what we query at runtime — so ours passes through the backend, where tests drive it. |
| 25 | `results.rs` / `results.test.ts` | `an_image_with_no_text_resolves_with_empty_text_and_no_blocks` | an image with no text resolves with empty text and no blocks | ML Kit and Vision return empty text with no blocks when nothing is recognized (the iOS plugin's empty result); that is success, not an error. |
| 26 | `results.rs` / `results.test.ts` | `unicode_text_passes_through_unchanged` | Unicode text passes through unchanged | ML Kit returns UTF-8 text for Chinese, Japanese, Korean and Devanagari; our Rust `String` and TS string pass it through unchanged. |

### Known gaps

Cases the references handle that no matrix row above tests; all untested:

- Non-empty `Encoded` bytes a platform decoder cannot decode: the references fail this inside the platform (ML Kit's `InputImage`, `CGImageSource`, `BitmapDecoder`), and our contract makes it the backend's `ERR_OCR_INVALID_IMAGE`; the matrix only refuses empty or mis-shaped input at validation (rows 12-14).
- The platform conversions — Vision's normalized bottom-left boxes to pixel top-left rects, Windows' RGBA-to-BGRA — mirror the references' mapping layers and are covered only by inline unit tests inside the platform files, which run on the one runner that compiles each platform.
- No CI runner holds a real engine or model files: every matrix row drives `FakeBackend` or `memoryBackend`, and CI has no emulator or device job, so actual recognition on Apple, Android, Windows, Linux or web is untested in this repo. Android alone has a manual path: CI packages `examples/demo` into an installable APK (the run's `demo-android` artifact, arm64, Latin model only) so recognition can be checked on a real device by hand.
