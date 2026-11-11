# Parity with `google_mlkit_text_recognition` and `react-native-mlkit-text-recognition`

Each row names one API of Flutter's `google_mlkit_text_recognition` 1.0.1 (the names come from `packages/google_mlkit_text_recognition` and `packages/google_mlkit_commons` in the google-ml-kit-flutter checkout under `.refs/`, read on 2026-11-09), one API of the React Native module `react-native-mlkit-text-recognition` 6.0.0, or one platform capability both references rely on, the status here, and the test that covers it.

Status words:

- **Done**: works on every platform the row names, as the reference does.
- **Partial**: works, with a limit named in the row.
- **Accepted**: the name is accepted and passed on, but the platform does nothing with it.
- **Platform only**: the effect exists on a device and no CI runner can show it; the test checks what the library passes on.

Every row is written against the frozen design; no row is verified until CI runs its test.

Rust test names are in `crates/nsh-ocr/tests/`. TypeScript tests are in `packages/nsh-ocr/tests/`.

## Methods of Flutter's `TextRecognizer`

| Flutter | Rust | TypeScript | Status | Covered by |
|---|---|---|---|---|
| `TextRecognizer({ script })`, the script fixed per instance, `latin` the default | `TextRecognizer::new()`; the script travels with each call in `RecognitionOptions.script` | `new TextRecognizer({ script })` keeps a default script; `processImage(image, { script })` sets it per call | Partial. No recognizer here is bound to one script; a closed-over default exists only on the TypeScript side | `the_script_and_options_reach_the_backend`; "the script and options reach the backend" |
| `processImage(InputImage)` | `TextRecognizer::process_image(&ImageInput, &RecognitionOptions)` | `TextRecognizer.processImage(image, options?)` | Done. Concurrent calls are allowed; each call resolves exactly once | `a_call_resolves_once_with_the_recognized_text`; `concurrent_calls_each_resolve_once`; "a call resolves once with the recognized text" |
| `close()` releases the native recognizer; a later call silently rebuilds it | `TextRecognizer::close()`, idempotent; a closed recognizer refuses `process_image` | `TextRecognizer.close()` | Partial. The first close also resolves in-flight calls with `ERR_OCR_CANCELLED`, and a closed recognizer stays closed (`ERR_OCR_CLOSED`), where Flutter would rebuild | `close_is_idempotent`; `close_resolves_in_flight_calls_as_cancelled`; `a_closed_recognizer_refuses_process_image_with_the_closed_code`; "close is idempotent" |
| not in Flutter | `TextRecognizer::cancel() -> bool` | `TextRecognizer.cancel(): Promise<boolean>` | Done. Added here. Resolves every pending call with `ERR_OCR_CANCELLED`; `false` when nothing waits | `cancel_resolves_pending_calls_with_the_cancelled_code`; `cancel_with_no_pending_call_reports_false`; "cancel resolves pending calls with the cancelled code" |
| not in Flutter | `RecognitionOptions.timeout_ms`, opt-in; `Some(0)` is refused before any work starts | `timeoutMs` | Done. Added here. `Some(ms)` races the answer and wins with `ERR_OCR_TIMEOUT`; the late answer is dropped | `a_timeout_resolves_with_the_timeout_code_and_the_late_answer_is_dropped`; `zero_timeout_is_refused_as_an_invalid_argument`; "a timeout resolves with the timeout code and the late answer is dropped" |
| `timeout_ms` on wasm32 | accepted and unused; the timer is compiled out with `cfg` | `timeoutMs` still runs the TypeScript watchdog | Accepted. The option crosses the bridge; the Rust core ignores it on wasm32, where tokio has no time driver | "a timeout resolves with the timeout code and the late answer is dropped" in `recognizer.test.ts` |
| not in either reference | `TextRecognizer::supported_languages()` | `TextRecognizer.supportedLanguages()` | Done. Added here. The runtime list: Vision `supportedRecognitionLanguages`, Windows `AvailableRecognizerLanguages`, Android the five script languages, ocrs `["en"]` | `supported_languages_pass_through_the_backend`; "supported languages pass through the backend" |

## The result tree

| Flutter | Rust | TypeScript | Status | Covered by |
|---|---|---|---|---|
| `RecognizedText { text, blocks }` | `RecognizedText { text, blocks }` | `RecognizedText` | Done. `text` joins all recognized text; an image with no text resolves `Ok` with empty text and no blocks, which is success, not an error | `an_image_with_no_text_resolves_with_empty_text_and_no_blocks`; `unicode_text_passes_through_unchanged`; "an image with no text resolves with empty text and no blocks" |
| `TextBlock { text, lines, boundingBox, recognizedLanguages, cornerPoints }` | `TextBlock { text, lines, bounding_box, languages, corner_points }` | camelCase mirror | Done, with two renames: `recognizedLanguages` is `languages` here, and the box is our `Rect` below | `the_result_tree_mirrors_the_reference_shapes`; "the result tree mirrors the reference shapes" |
| `TextLine { text, elements, boundingBox, recognizedLanguages, cornerPoints, confidence, angle }`, both null on iOS | `TextLine`, with `confidence: Option<f32>` and `angle: Option<f32>` | `confidence` and `angle` as `number \| null` | Done. Android ML Kit fills both; Apple Vision gives confidence, no angle; Windows and ocrs give neither | `the_result_tree_mirrors_the_reference_shapes` |
| `TextElement { text, symbols, boundingBox, recognizedLanguages, cornerPoints, confidence, angle }`, symbols empty on iOS | `TextElement`, same fields plus the `symbols` list | camelCase mirror | Done. Android ML Kit produces elements and symbols; ocrs fills elements from words with no symbols; Apple Vision emits lines with no elements at all | `the_result_tree_mirrors_the_reference_shapes` |
| `TextSymbol { text, boundingBox, recognizedLanguages, cornerPoints, confidence, angle }` | `TextSymbol` with the same fields | camelCase mirror | Done. Android only; the field keeps its shape everywhere with empty lists and nulls elsewhere | `the_result_tree_mirrors_the_reference_shapes` |
| `boundingBox`: a dart `Rect`, sent by both plugins as `{left, top, right, bottom}` | `Rect { left, top, right, bottom }`, pixels, top-left origin | `Rect { left, top, right, bottom }` | Done. Apple converts Vision's normalized bottom-left box to this in a pure function with inline unit tests | `the_result_tree_mirrors_the_reference_shapes` |
| `cornerPoints`, clockwise from top-left | `corner_points: Vec<Point>`, `Point { x, y }` | `cornerPoints: Point[]` | Partial. Filled where the platform gives points (Android ML Kit `cornerPoints`, ocrs word corners); Apple and Windows name none in the frozen mapping, so the list is empty there | `the_result_tree_mirrors_the_reference_shapes` |
| `recognizedLanguages`: Android one `recognizedLanguage` string per level, iOS a list | `languages: Vec<String>` on every level | `languages: string[]` | Done. A list everywhere, one entry where the platform gives one | `the_result_tree_mirrors_the_reference_shapes` |

## `InputImage` forms

| Flutter | Rust | TypeScript | Status | Covered by |
|---|---|---|---|---|
| `InputImage.fromFilePath(path)` | no path variant in the core; `NativeImageInput.path` is read with `std::fs` into `Encoded(bytes)` | `{ path: string }` | Done. A failed read is `ERR_OCR_IO_FAILED`; an empty path is refused at validation as `ERR_OCR_INVALID_IMAGE` | none beyond the read in `native.rs` |
| `InputImage.fromBytes(bytes, metadata)`: NV21, YUV_420_888 or YV12 on Android, YUV420 or BGRA8888 on iOS, with size, rotation and bytesPerRow | `ImageInput::Encoded(Vec<u8>)`, a format the platform decodes (PNG, JPEG, ...) | `{ encoded: Uint8Array }` | Partial. No camera-plane formats, no rotation, no row stride; bytes a platform cannot decode come back `ERR_OCR_INVALID_IMAGE` from the backend | `empty_encoded_bytes_are_refused_as_an_invalid_image`; "empty encoded bytes are refused as an invalid image" |
| `InputImage.fromBitmap(bitmap, width, height, rotation)`: raw pixels from the Flutter UI | `ImageInput::Rgba { width, height, data }`, RGBA8, row-major, top-left, `data.len() == width * height * 4` | `RgbaImage { width, height, data }` | Partial. Same pixels, no rotation parameter | `an_rgba_size_mismatch_is_refused_as_an_invalid_image`; `zero_dimensions_are_refused_as_an_invalid_image`; "an RGBA size mismatch is refused as an invalid image" |

## `TextRecognitionScript`

| Flutter | Rust | TypeScript | Status | Covered by |
|---|---|---|---|---|
| `latin`, the default | `Script::Latin`, `name()` `"latin"`, primary language `en-US` | `"latin"` | Done | `each_script_has_the_name_tsx_uses`; "each script has the name TSX uses" |
| `chinese` | `Script::Chinese`, `"chinese"`, `zh-Hans` | `"chinese"` | Done | `each_script_has_the_name_tsx_uses`; "each script has the name TSX uses" |
| `devanagiri`, Flutter's spelling | `Script::Devanagari`, `"devanagari"`, `hi-IN` | `"devanagari"` | Done, with the standard spelling here; an unknown script name over the bridge is `ERR_OCR_INVALID_ARGUMENT` | `each_script_has_the_name_tsx_uses`; "each script has the name TSX uses" |
| `japanese` | `Script::Japanese`, `"japanese"`, `ja-JP` | `"japanese"` | Done | `each_script_has_the_name_tsx_uses`; "each script has the name TSX uses" |
| `korean` | `Script::Korean`, `"korean"`, `ko-KR` | `"korean"` | Done | `each_script_has_the_name_tsx_uses`; "each script has the name TSX uses" |

## The `react-native-mlkit-text-recognition` module

| The module | Rust | TypeScript | Status | Covered by |
|---|---|---|---|---|
| `recognizeText(imagePath): Promise<Text>`, Latin options only on both platforms | `process_image(&ImageInput, &RecognitionOptions)`; `imagePath` is the `path` variant of the input | `processImage({ path })`, any script | Done. The path is read on the Rust side; the script is a per-call option | `a_call_resolves_once_with_the_recognized_text`; "a call resolves once with the recognized text" |
| `Text { text, blocks }` | `RecognizedText` | `RecognizedText` | Done | `the_result_tree_mirrors_the_reference_shapes`; "the result tree mirrors the reference shapes" |
| `Block { text, frame, recognizedLanguages, lines }` | `TextBlock`; `frame` is `bounding_box`, `recognizedLanguages` is `languages` | camelCase mirror | Done, with the two renames | `the_result_tree_mirrors_the_reference_shapes` |
| `TextLine { text, frame, recognizedLanguages, elements }` | `TextLine`, plus `corner_points`, `confidence`, `angle` | camelCase mirror | Done, with more fields here where a platform gives them | `the_result_tree_mirrors_the_reference_shapes` |
| `TextElement { text, frame, recognizedLanguages }` | `TextElement`, plus `symbols`, `corner_points`, `confidence`, `angle` | camelCase mirror | Done, with more fields here | `the_result_tree_mirrors_the_reference_shapes` |
| `Rect { left, top, right, bottom }`, the `frame` | `Rect { left, top, right, bottom }` | `Rect` | Done | `the_result_tree_mirrors_the_reference_shapes` |
| no `close()`, no cancel | `close()`, `cancel()` | `close()`, `cancel()` | Done. Added here | `close_is_idempotent`; `cancel_resolves_pending_calls_with_the_cancelled_code`; "close is idempotent" |
| no confidence, no angle, no symbols | `confidence`, `angle`, `symbols` | `number \| null`, `TextSymbol[]` | Platform only. Android fills them; Apple Vision gives confidence only; Windows and ocrs give neither | `the_result_tree_mirrors_the_reference_shapes` |

## Platform capabilities

| Capability | Rust | TypeScript | Status | Covered by |
|---|---|---|---|---|
| Apple Vision `VNRecognizeTextRequest` at the accurate level | `platform/apple.rs`; the script's primary language is resolved against the runtime `supportedRecognitionLanguages` for the accurate revision before anything starts; missing is `ERR_OCR_UNSUPPORTED_LANGUAGE`; each observation maps to one `TextLine`, confidence set, angle not, elements and symbols empty | the same call through `rustBackend()` | Platform only. The rect and coordinate mapping are pure functions with inline unit tests in `apple.rs`, kept free of Vision types so they parse anywhere | `an_unsupported_script_resolves_with_the_unsupported_language_code`; "an unsupported script resolves with the unsupported language code" |
| Apple Vision cannot stop running work | `cancel_recognition` is a no-op in `apple.rs` | `cancel()` still resolves the call | Partial. Vision finishes its work regardless; the core drops the late answer after a cancel or a close | `a_late_answer_after_a_cancel_is_dropped`; `dropping_the_call_cancels_the_backend`; "a late answer after a cancel is dropped" |
| Android ML Kit v2: one recognizer per script (Latin, Chinese, Devanagari, Japanese, Korean) | `platform/android.rs`; the script maps to the matching options class; Encoded decodes through `BitmapFactory.decodeByteArray`, Rgba through `Bitmap.createBitmap` ARGB_8888, then `InputImage.fromBitmap(bitmap, 0)`; exactly one answer through the task listeners, a failed task is a coded error | the same call through `rustBackend()` | Platform only | `a_backend_engine_failure_resolves_with_the_backend_code`; "a backend engine failure resolves with the backend code" |
| Android ML Kit v2: confidence and angle on lines and elements | `TextLine` and `TextElement` carry both; symbols too | `number \| null` | Platform only | `the_result_tree_mirrors_the_reference_shapes` |
| Android answers the five script languages | `supported_languages` returns all five | `supportedLanguages()` | Platform only | `supported_languages_pass_through_the_backend`; "supported languages pass through the backend" |
| Windows.Media.Ocr: `AvailableRecognizerLanguages` at runtime | `platform/windows.rs`; `TryCreateFromLanguage` with the `IsLanguageSupported` fallback, missing is `ERR_OCR_UNSUPPORTED_LANGUAGE`; `supported_languages` answers the runtime list | the same call through `rustBackend()` | Platform only | `an_unsupported_script_resolves_with_the_unsupported_language_code`; `supported_languages_pass_through_the_backend` |
| Windows.Media.Ocr: no confidence, no angle, cannot stop running work | `confidence` and `angle` stay `None`; `cancel_recognition` is a no-op | `null` fields; `cancel()` still resolves the call | Partial, as with Vision: the late answer is dropped | `a_late_answer_after_a_cancel_is_dropped` |
| Windows bitmaps are BGRA | Rgba input is converted RGBA to BGRA in a pure, unit-tested function; Encoded goes through `BitmapDecoder`, undecodable bytes are `ERR_OCR_INVALID_IMAGE` | the same input through `rustBackend()` | Platform only. The conversion is an inline unit test in `windows.rs` | none beyond the conversion test in `windows.rs` |
| ocrs 0.14: the app ships the model files | `OcrsBackend::from_model_bytes(detection, recognition)` parses both eagerly through rten, a parse failure is `ERR_OCR_MODEL_NOT_LOADED`; `use_ocrs_models` sets the process-wide bytes the default backend uses, a later call replaces them | `rustBackend()` on Linux and web; without models `processImage` rejects with `ERR_OCR_MODEL_NOT_LOADED` | Done. `supported_languages` still answers `["en"]` without models | `a_model_error_resolves_with_the_model_code`; "a model error resolves with the model code" |
| ocrs 0.14: Latin only | a non-Latin script is `ERR_OCR_UNSUPPORTED_LANGUAGE` before any work starts; `supported_languages` is `["en"]` | the same call through `rustBackend()` | Partial. Latin only on Linux and web | `an_unsupported_script_resolves_with_the_unsupported_language_code`; "an unsupported script resolves with the unsupported language code" |
| ocrs 0.14: lines of words, no blocks, no confidence | `OcrEngine::prepare_input`, then `get_text` for the joined text and `recognize_text` for lines; each ocrs line is one `TextLine`, words become elements with boxes from the word rects and corners from the word corners, all under one `TextBlock` per image; Encoded decodes through the image crate | the same call through `rustBackend()` | Done, with the one-block-per-image limit named; `confidence` and `angle` are `None` | `the_result_tree_mirrors_the_reference_shapes`; `an_image_with_no_text_resolves_with_empty_text_and_no_blocks` |
| any other target | `platform/unsupported.rs`: `start_recognition` is `ERR_OCR_NOT_AVAILABLE`, `supported_languages` is `Ok(vec![])` | the same call through `rustBackend()` | Done | `every_kind_reaches_the_caller_with_its_code`; "every kind reaches the caller with its code" |

## Errors

The references surface failures as opaque plugin errors (`TextRecognizerError` with the exception text on Android, `FlutterError` codes or `NSError` on iOS, `CodedException` in the React Native module). Every failure mode maps to one code here. The mapping lives in `crates/nsh-ocr/src/error.rs` and the bridge files.

| Reference surface | Code here | Produced by | Status | Covered by |
|---|---|---|---|---|
| Corrupt or wrong-shape image: iOS `invalid_image` ("Invalid or missing image data"), Android rejects what `InputImageConverter` cannot build | `ERR_OCR_INVALID_IMAGE` | Every platform. The core validates first (empty encoded bytes, a zero dimension, an RGBA length mismatch); each bridge rejects encoded bytes it cannot decode | Done | `an_rgba_size_mismatch_is_refused_as_an_invalid_image`; `zero_dimensions_are_refused_as_an_invalid_image`; `empty_encoded_bytes_are_refused_as_an_invalid_image`; "an RGBA size mismatch is refused as an invalid image" |
| Unreadable path: iOS folds a failed `UIImage(contentsOfFile:)` into `invalid_image`, Android answers `InputImageConverterError` when `InputImage.fromFilePath` throws; neither separates a bad path from a bad image | `ERR_OCR_IO_FAILED` | The `native.rs` bridge, when the `std::fs` read of a path input fails | Done | none beyond the read in `native.rs` |
| Unsupported language or script: no reference code; Vision resolves against runtime `supportedRecognitionLanguages`, Windows against the engine's languages, ocrs is Latin only | `ERR_OCR_UNSUPPORTED_LANGUAGE` | Apple, Windows, Linux and web | Done | `an_unsupported_script_resolves_with_the_unsupported_language_code`; "an unsupported script resolves with the unsupported language code" |
| Engine failure: Android `addOnFailureListener`, iOS the `NSError` from `process`, the React Native module's `CodedException` | `ERR_OCR_BACKEND` | Every platform: a failed ML Kit task, a Vision request error, a Windows engine failure, an ocrs inference failure | Done, with the platform status where one is given | `a_backend_engine_failure_resolves_with_the_backend_code`; `a_platform_status_is_kept_with_its_error`; "a backend engine failure resolves with the backend code" |
| Model problems: ocrs `DetectionModelNotLoaded`, `RecognitionModelNotLoaded`, `IncompatibleModel`; never surfaced by either reference, which ship their models with the app | `ERR_OCR_MODEL_NOT_LOADED` | Linux and web: models never passed to `use_ocrs_models`, or model bytes that fail to parse | Done | `a_model_error_resolves_with_the_model_code`; "a model error resolves with the model code" |
| Closed recognizer: no reference code; Flutter's plugin rebuilds the recognizer after `close()` | `ERR_OCR_CLOSED` | The core, on every platform | Done | `a_closed_recognizer_refuses_process_image_with_the_closed_code`; "a closed recognizer refuses processImage with the closed code" |
| Cancel: no reference can cancel recognition | `ERR_OCR_CANCELLED` | The core: `cancel()`, the first `close()`, a dropped call future. Vision and Windows cannot stop running work; the late answer is dropped | Done | `cancel_resolves_pending_calls_with_the_cancelled_code`; `close_resolves_in_flight_calls_as_cancelled`; `dropping_the_call_cancels_the_backend`; "cancel resolves pending calls with the cancelled code" |
| Malformed arguments: the iOS plugin answers `invalid_args` (`FlutterError` for a missing `imageData` or `id`); here the refused ones are a zero timeout and an unknown script name | `ERR_OCR_INVALID_ARGUMENT` | The core and the bridge: `timeout_ms == Some(0)` refused before any work starts, an unknown script name over the bridge | Done | `zero_timeout_is_refused_as_an_invalid_argument`; "a zero timeout is refused as an invalid argument" |
| A timed-out recognition: no reference surface | `ERR_OCR_TIMEOUT` | The core on non-wasm32 through tokio; the TypeScript watchdog everywhere | Done | `a_timeout_resolves_with_the_timeout_code_and_the_late_answer_is_dropped`; "a timeout resolves with the timeout code and the late answer is dropped" |
| No recognition engine on the target: no reference surface | `ERR_OCR_NOT_AVAILABLE` | Targets with no bridge (`platform/unsupported.rs`) | Done | `every_kind_reaches_the_caller_with_its_code`; "every kind reaches the caller with its code" |

The set is held together by `every_kind_reaches_the_caller_with_its_code`, `every_kind_has_a_distinct_code` and `a_platform_status_is_kept_with_its_error` ("every kind reaches the caller with its code", "every kind has a distinct code", "a platform status is kept with its error" in `errors.test.ts`).
