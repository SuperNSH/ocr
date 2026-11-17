# Audit

Every claim in this repo's planning documents, checked against the code and tests on `master` at `3d55e31` (2026-11-17). Evidence is a file:line I read, a test body, or a read-only command (`gh run list`, `gh release list`, `curl -sI`). Nothing was built or run.

CI evidence used throughout: `gh run list --limit 5` shows CI green on `master` — run 44000499013 (2026-11-17, "build: move to NSH 0.5.1"), 43202528722 (2026-11-12), 42958520901 (2026-11-11) — and Pages green (44000498357, 2026-11-17). So every test named below has a passing run behind it.

Statuses: **Done** (code exists, a test exercises it), **Untested** (code exists, no test proves it), **Partial** (some of it exists), **Missing** (none of it exists), **Blocked** (needs a change outside this repo), **Stale** (the doc is wrong and the code is right).

Two facts that shape many rows:

- The TypeScript tests are registered with `test()` from `nsh/test` (e.g. `packages/nsh-ocr/tests/recognizer.test.ts:1`), not `it()` from vitest. The phrases are the exact frozen names; the harness is NSH's own. Rows below cite the phrase and the line.
- While this audit ran, other agents landed uncommitted changes in the working tree (a `useOcrsModels` native bridge, explicit `onnx_format`, a multi-target demo job with `nsh test`, pages/site removal). None of that is on `master` and none of it has run CI; every row below records `master` at `3d55e31`, and the in-flight work is named only where it directly answers a gap row.

## README.md

| # | Claim | Where it should live | Evidence | Status |
|---|---|---|---|---|
| 1 | "a web status page on GitHub Pages at https://supernsh.github.io/ocr/" (README.md:5) | `.github/workflows/pages.yml`, `site/` | pages.yml deploys `site/` on push to `master`; Pages run 44000498357 green; `curl -sI` answers 200 (last-modified 2026-11-17) | Done |
| 2 | "**Implemented, awaiting CI.** … No CI run has compiled or tested any of it yet, so no behaviour is verified." (README.md:9) | `gh run list` | Run 44000499013 green on `master` 2026-11-17; green CI runs since 2026-11-11 (42958520901, 43202528722). The line violates AGENTS.md's "status claims must match what CI runs" | Stale |
| 3 | "ten coded errors, image input as encoded bytes or RGBA pixels, five scripts, a recognizer whose calls resolve exactly once" (README.md:9) | `crates/nsh-ocr/src/error.rs`, `packages/nsh-ocr/src/errors.ts`, `src/input.rs`, `src/options.rs`, `src/recognizer.rs` | Codes/messages identical in error.rs:74-101 and errors.ts:15-26; resolve-once proven by matrix rows 1-11 in both languages; five scripts options.rs:25-43 (the input also takes a `path`, which the line omits — not false, incomplete) | Done |
| 4 | "bridges to Apple Vision, Android ML Kit, Windows.Media.Ocr and ocrs" (README.md:9) | `crates/nsh-ocr/src/platform/{apple,android,windows,ocrs}.rs` | All four bridges exist and compile green in CI's cross-check; no test drives any of them (goals.md:85 says so itself) | Untested |
| 5 | "Vision on Apple, queried at runtime for supported languages. ocrs elsewhere, with pinned versions." (README.md:13) | `platform/apple.rs`, `Cargo.toml` | Runtime query apple.rs:34-41; versions pinned (ocrs 0.14.0, rten 0.28, Cargo.toml:46-51). "ocrs elsewhere" is wrong for Android (ML Kit) and Windows (Media.Ocr) — decisions.md:28-31 is the corrected story | Partial |
| 6 | Layout bullets: `crates/nsh-ocr/`, `packages/nsh-ocr/`, `examples/demo/` packaged by CI into an APK, docs files, `site/`, workflows (README.md:17-22) | the tree | Every named path exists; `docs/` holds exactly brief/decisions/goals/parity | Done |
| 7 | "Every compile and test runs in GitHub Actions. Nothing is built on a developer machine." (README.md:26) | `gh run list` | Green runs cover fmt/clippy/test/cross-check/nsh check/build/test and the demo job. Note: untracked `target/` and `examples/demo/target/` build dirs exist on this machine (parallel work; the tracked tree is clean) | Done |
| 8 | "the merged APK is uploaded as the run's `demo-android` artifact (arm64, Latin model only — the other scripts answer a coded error on the device)" (README.md:28) | `.github/workflows/ci.yml:99-155`, `examples/demo/android/` | Artifact named `demo-android` with `if-no-files-found: error` (ci.yml:151-155); `--abi arm64-v8a` (ci.yml:125); Gradle carries only `com.google.mlkit:text-recognition:16.0.1` (app/build.gradle:32); a missing script class fails JNI lookup and maps to a coded error (android.rs:522-556, 69-73) | Done |
| 9 | "MIT. See LICENSE." (README.md:32) | `LICENSE` | Present | Done |
| 10 | "Packages `examples/demo` for Android … so recognition can be checked on a real device by hand" (README.md:28) | demo job | Job ran green in 44000499013; no automated check of the APK's recognition exists — the word "by hand" is accurate | Done |

## docs/goals.md — intro and rules

| # | Claim | Where | Evidence | Status |
|---|---|---|---|---|
| 1 | Engines per platform; "every other target answers `ERR_OCR_NOT_AVAILABLE`" (goals.md:3) | `src/platform/mod.rs` | Dispatch matches exactly (mod.rs:28-46); unsupported.rs:26 returns the code; no test drives a non-bridge target (row 15 tests only the error's plumbing) | Partial |
| 2 | The two references' API shapes (goals.md:7-8) | `.refs/` | `.refs/google_ml_kit_flutter/.../text_recognizer.dart:41` (`devanagiri` spelling, the five scripts); `.refs/react-native-mlkit/.../RNMLKitTextRecognitionModule.ts:36` (`recognizeText: (imagePath: string) => Promise<Text>`) | Done |
| 3 | Gates: fmt; clippy+test on three OS, fail-fast off; cross-target clippy for the four targets with the NDK block (goals.md:15-17) | `ci.yml:23-77` | All present verbatim; strategies set `fail-fast: false` (ci.yml:39, 57); NDK env block ci.yml:70-76 | Done |
| 4 | TypeScript gate: "under NSH v0.5.1: `nsh check`; `nsh build` with no warning in its log; `nsh test`" (goals.md:18) | `ci.yml:79-97` | `NSH_VERSION: v0.5.1`, `working-directory: packages/nsh-ocr`, `! grep -i warn build.log` — all present | Done |
| 5 | "Resolve exactly once … a late platform answer that arrives after the call resolved is dropped" (goals.md:22) | `src/recognizer.rs`, `src/recognizer.ts` | Generation registry + drop of stale answers (recognizer.rs:95-139, 193-231); proven by rows 1, 5, 6, 8 in both languages | Done |
| 6 | "No panics in any bridge" (goals.md:23) | platform files | catch_unwind in apple.rs:285, windows.rs:313, ocrs.rs:189, android.rs:714; no CI runner reaches a bridge, so no test can show it | Untested |
| 7 | "Every public API of the two references has a row in `docs/parity.md`" (goals.md:24) | `docs/parity.md` | The parity tables below cover the Flutter/RN surfaces named in `.refs/` | Done |
| 8 | Layout rule: same names, same behaviour, same error wording in Rust and TSX (goals.md:25) | error.rs vs errors.ts | Ten codes and ten messages byte-identical (error.rs:74-101, errors.ts:15-26); pinned by error.rs unit test `every_kind_has_its_frozen_code_and_message` and matrix rows 15-16 | Done |
| 9 | "Nothing in this repo is verified until a run on `master` passes; until then every status claim … is unverified" (goals.md:27) | `gh run list` | A run passed (44000499013, 2026-11-17); the tests this doc names have run green. Same staleness as README row 2 | Stale |

## docs/goals.md — the Errors table

Codes and messages: all ten identical in error.rs:74-101 and errors.ts:15-26 (verified string by string). The rows below judge the "when it happens" column.

| # | Kind / claim | Where | Evidence | Status |
|---|---|---|---|---|
| 1 | NotAvailable: every call from the unsupported backend; never on the five engine targets (goals.md:35) | `platform/unsupported.rs`, `platform/android.rs` | unsupported.rs:26; Android preflight for a missing ML Kit/bridge class android.rs:343-351. No test drives either producer | Partial |
| 2 | UnsupportedLanguage: Apple runtime list check before start; Windows `TryCreateFromLanguage` failure; Linux/web non-Latin; never Android (goals.md:36) | apple.rs, windows.rs, ocrs.rs | apple.rs:275-280 (check before thread starts); windows.rs:248-253; ocrs.rs:66-72 with unit test `a_non_latin_script_is_refused_before_any_work_starts` (runs on Linux). Apple/Windows paths compile only | Partial |
| 3 | ModelNotLoaded: `use_ocrs_models` never called or bytes fail to parse; Linux/web only (goals.md:37) | ocrs.rs | parse failure → model_not_loaded (ocrs.rs:52-64); lazy engine without stored bytes (ocrs.rs:142-160); unit tests `garbage_model_bytes_fail_with_model_not_loaded`, `native_backend_without_models_reports_model_not_loaded` run on Linux; matrix row 20 both languages. On Linux/web this is the permanent state for TSX apps — see Gap G1 | Done |
| 4 | InvalidImage: core validation (empty Encoded, zero dim, RGBA mismatch), then per-platform decode failures (goals.md:38) | input.rs, four bridges | input.rs:21-43 + unit tests + matrix rows 12-14 (core: Done). Decode refusals: ocrs unit-tested (`decode_encoded_rejects_garbage_as_invalid_image`, Linux); apple (apple.rs:45-59), android (android.rs:477-479), windows (windows.rs:36-40, 212-218) compile only | Partial |
| 5 | InvalidArgument: `timeout_ms == Some(0)` everywhere; "on TSX, a script name outside the five" (goals.md:39) | recognizer.rs, native.rs, recognizer.ts | Zero timeout refused before work (recognizer.rs:94-96; recognizer.ts:50-52); row 7 both languages. Unknown script is refused in the Rust bridge (native.rs:187-193, unit test at native.rs:258) — the TS package itself does no runtime script check; TS callers are guarded only by the `Script` type | Partial |
| 6 | IoFailed: reading the `path` fails, native TSX layer only; the Rust core never raises it (goals.md:40) | native.rs | native.rs:171-175; unit test `a_path_that_cannot_be_read_is_an_io_failure` (native.rs:325) runs on all three host OSes; no `IoFailed` path exists in the core | Done |
| 7 | Closed: `processImage` after `close()`, in the core (goals.md:41) | recognizer.rs, recognizer.ts | recognizer.rs:100-102; recognizer.ts:53; matrix row 9 both languages | Done |
| 8 | Cancelled: `cancel()` with a call pending, or `close()` in flight; no platform reports it (goals.md:42) | recognizer.rs, recognizer.ts | recognizer.rs:143-184; recognizer.ts:113-119; rows 3 and 11 both languages | Done |
| 9 | Timeout: Rust timer on non-wasm32, TS watchdog on wasm32; late answer dropped (goals.md:43) | recognizer.rs, recognizer.ts | tokio select with cfg (recognizer.rs:121-136); TS `setTimeout` watchdog (recognizer.ts:69-74); row 6 both languages; wasm32 compiles green in cross-check | Done |
| 10 | Backend: Vision request failure, ML Kit Task failure, `RecognizeAsync` failure, ocrs `InferenceFailed` (goals.md:44) | four bridges | apple.rs:119, windows.rs:31-33, android.rs:216-223 (`map_task_failure` keeps the MlKit code), ocrs.rs:46-50. Only the plumbing is tested (row 19); android's mapping unit test exists but never runs in CI (see Known gaps row 2) | Partial |

## docs/goals.md — the 26-row matrix

All 26 Rust fns exist with exactly the frozen names in `crates/nsh-ocr/tests/{recognizer,errors,results}.rs`; all 26 TS phrases exist verbatim as `test()` names in `packages/nsh-ocr/tests/*.test.ts`. Bodies were read; each asserts what its row promises unless named below.

| # | Rust fn (file:line) | TS phrase (file:line) | What the bodies prove | Status |
|---|---|---|---|---|
| 1 | `a_call_resolves_once_with_the_recognized_text` (recognizer.rs:9) | "a call resolves once with the recognized text" (recognizer.test.ts:35) | One answer resolves the call; a second `finish_recognition` returns false in both languages | Done |
| 2 | `concurrent_calls_each_resolve_once` (recognizer.rs:28) | (recognizer.test.ts:50) | Two pending calls, two answers, no third resolution; `started() == 2` | Done |
| 3 | `cancel_resolves_pending_calls_with_the_cancelled_code` (recognizer.rs:48) | (recognizer.test.ts:67) | Both pending calls reject `ERR_OCR_CANCELLED`; `cancels() >= 1` | Done |
| 4 | `cancel_with_no_pending_call_reports_false` (recognizer.rs:70) | "cancel with nothing pending reports false" (recognizer.test.ts:80) | `false` with nothing pending | Done |
| 5 | `a_late_answer_after_a_cancel_is_dropped` (recognizer.rs:78) | (recognizer.test.ts:85) | The stale answer does not resolve a newer call; both languages assert `!next.is_finished()` / the next call still waits | Done |
| 6 | `a_timeout_resolves_with_the_timeout_code_and_the_late_answer_is_dropped` (recognizer.rs:105) | (recognizer.test.ts:121) | Rust: real 50 ms sleep against a never-answering fake; TS: real 25 ms timeout waited out. Late answer dropped in both | Done |
| 7 | `zero_timeout_is_refused_as_an_invalid_argument` (recognizer.rs:129) | (recognizer.test.ts:140) | `ERR_OCR_INVALID_ARGUMENT`, `started() == 0` | Done |
| 8 | `dropping_the_call_cancels_the_backend` (recognizer.rs:148) | "dropping the call asks the backend to cancel" (recognizer.test.ts:102) | Rust: `call.abort()` drops the future, `cancels() == 1`, stale answer dropped — exactly the row. TS: a promise cannot be dropped; the body drives `cancel()` and asserts `cancels() == 1`. It proves cancel asks the backend to stop, not drop semantics | Partial |
| 9 | `a_closed_recognizer_refuses_process_image_with_the_closed_code` (recognizer.rs:173) | (recognizer.test.ts:149) | `ERR_OCR_CLOSED`, `started() == 0`, refused before the backend | Done |
| 10 | `close_is_idempotent` (recognizer.rs:189) | (recognizer.test.ts:157) | Two closes; the second asks the backend nothing new (`cancels() == 1` in both) | Done |
| 11 | `close_resolves_in_flight_calls_as_cancelled` (recognizer.rs:216) | (recognizer.test.ts:169) | Both in-flight calls reject cancelled; later calls reject closed | Done |
| 12 | `an_rgba_size_mismatch_is_refused_as_an_invalid_image` (errors.rs:27) | (errors.test.ts:34) | 15 bytes for 2x2 rejected, `started() == 0` | Done |
| 13 | `zero_dimensions_are_refused_as_an_invalid_image` (errors.rs:49) | (errors.test.ts:45) | Both zero-width and zero-height rejected | Done |
| 14 | `empty_encoded_bytes_are_refused_as_an_invalid_image` (errors.rs:78) | (errors.test.ts:58) | Empty rejected; non-empty passes validation and reaches the backend | Done |
| 15 | `every_kind_reaches_the_caller_with_its_code` (errors.rs:103) | (errors.test.ts:72) | All ten kinds delivered with their exact codes | Done |
| 16 | `every_kind_has_a_distinct_code` (errors.rs:117) | (errors.test.ts:83) | Ten codes, ten uniques; messages non-empty | Done |
| 17 | `a_platform_status_is_kept_with_its_error` (errors.rs:133) | (errors.test.ts:90) | Rust: `with_status(-7)` survives to the caller. TS: the status never crosses the bridge; the body asserts code + frozen message only — weaker than the row's name | Partial |
| 18 | `an_unsupported_script_resolves_with_the_unsupported_language_code` (errors.rs:149) | (errors.test.ts:103) | `fail_next` with the kind; `started() == 0`; the refusal does not poison the recognizer | Done |
| 19 | `a_backend_engine_failure_resolves_with_the_backend_code` (errors.rs:173) | (errors.test.ts:116) | `ERR_OCR_BACKEND` delivered | Done |
| 20 | `a_model_error_resolves_with_the_model_code` (errors.rs:185) | (errors.test.ts:125) | `fail_next` model error; `started() == 0`; next call runs | Done |
| 21 | `the_result_tree_mirrors_the_reference_shapes` (results.rs:12) | (results.test.ts:26) | Full tree asserted level for level: block/line/element/symbol, boxes, corners, confidence/angle set only where a platform gives them (TS also asserts whole-tree equality) | Done |
| 22 | `each_script_has_the_name_tsx_uses` (results.rs:57) | (results.test.ts:78) | Five names and five primary languages, both languages (TS via `SCRIPT_LANGUAGES` keys) | Done |
| 23 | `the_script_and_options_reach_the_backend` (results.rs:74) | (results.test.ts:88) | Fake records Japanese + 60 000 ms timeout; TS checks `lastOptions()` | Done |
| 24 | `supported_languages_pass_through_the_backend` (results.rs:92) | (results.test.ts:100) | Passes the scripted list through; error path checked too | Done |
| 25 | `an_image_with_no_text_resolves_with_empty_text_and_no_blocks` (results.rs:108) | (results.test.ts:55) | Success with empty text and no blocks | Done |
| 26 | `unicode_text_passes_through_unchanged` (results.rs:125) | (results.test.ts:66) | CJK/em-dash/accented/emoji text unchanged through every level | Done |

## docs/goals.md — Known gaps

| # | Claim | Evidence | Status |
|---|---|---|---|
| 1 | Undecodable non-empty Encoded bytes are the backend's `ERR_OCR_INVALID_IMAGE`; "no matrix row above tests" them, "all untested" (goals.md:83) | True for apple/android/windows (compile only). The ocrs decoder refusal IS unit-tested on Linux: `decode_encoded_rejects_garbage_as_invalid_image` (ocrs.rs:602). "All untested" overstates | Partial |
| 2 | Platform conversions "covered only by inline unit tests … which run on the one runner that compiles each platform" (goals.md:84) | True for apple (macOS runs cargo test), windows (Windows runner), ocrs (Linux runner). False for android: `android.rs`'s `#[cfg(test)]` tests are compiled only under `target_os = "android"`, and no CI job runs `cargo test` for that target — cross-check runs clippy without `--all-targets` (ci.yml:77), so the test module is never even parsed | Partial |
| 3 | No CI runner holds a real engine or model files; no emulator or device job; Android alone has the manual APK path (goals.md:85) | ci.yml has no emulator/device job; the demo job packages only. Honest | Done |

## docs/parity.md

Intro line 12 ("no row is verified until CI runs its test") is **Stale** for the same reason as goals.md row 9: CI has run green (44000499013).

### Methods of Flutter's TextRecognizer

| # | Claim (parity.md:line) | Evidence | Status |
|---|---|---|---|
| 1 | Partial: no recognizer bound to one script; closed-over default only on TS (parity.md:20) | Script travels in `RecognitionOptions` per call; TS default recognizer.ts:29; row 23 both languages | Done |
| 2 | processImage Done, concurrent calls allowed, each resolves once (parity.md:21) | Rows 1-2 both languages | Done |
| 3 | close(): Partial — first close resolves in-flight with CANCELLED, stays closed where Flutter rebuilds (parity.md:22) | recognizer.rs:150-163; rows 9-11 both languages | Done |
| 4 | cancel() added, resolves pending with CANCELLED, false when nothing waits (parity.md:23) | Rows 3-4 both languages | Done |
| 5 | timeout_ms added, wins with TIMEOUT, late answer dropped (parity.md:24) | Rows 6-7 both languages | Done |
| 6 | wasm32: accepted and unused; TS watchdog runs (parity.md:25) | recognizer.rs:121-136 cfg; recognizer.ts:69-74; wasm32 compiles green in cross-check; TS timeout test green | Done |
| 7 | supported_languages(): runtime list per platform (parity.md:26) | Row 24 both languages; apple.rs:34-41, windows.rs:181-189, android.rs:202-213, ocrs.rs LANGUAGES | Done |

### The result tree

| # | Claim | Evidence | Status |
|---|---|---|---|
| 8 | RecognizedText; empty success (parity.md:32) | Row 25 both languages | Done |
| 9 | TextBlock with two renames: `languages`, our Rect (parity.md:33) | text.rs:42-54; row 21 | Done |
| 10 | TextLine confidence/angle optional per platform (parity.md:34) | text.rs:56-74; row 21 asserts `Some`/`None` per level | Done |
| 11 | TextElement plus symbols (parity.md:35) | text.rs:76-94; row 21 | Done |
| 12 | TextSymbol keeps its shape everywhere (parity.md:36) | text.rs:96-112; row 21 | Done |
| 13 | boundingBox a pixel top-left Rect; Apple conversion in a pure unit-tested function (parity.md:37) | `pixel_rect` apple.rs:158-165 with tests apple.rs:304-329 (run on macOS CI) | Done |
| 14 | cornerPoints: "Apple and Windows name none in the frozen mapping, so the list is empty there" (parity.md:38) | Wrong: apple.rs fills four synthesized corners on lines and blocks (apple.rs:147, 254); windows.rs fills them on lines, elements and blocks (windows.rs:137, 144, 166). The code is right, the row is not | Stale |
| 15 | recognizedLanguages → `languages: Vec<String>` at every level (parity.md:39) | text.rs; android maps `getRecognizedLanguage` into a one-entry list (android.rs:935-938) | Done |

### InputImage forms

| # | Claim | Evidence | Status |
|---|---|---|---|
| 16 | fromFilePath → path variant read with std::fs; "an empty path is refused at validation as ERR_OCR_INVALID_IMAGE" (parity.md:45) | TS: yes, input.ts:16. Rust: `NativeImageInput{path:""}` reaches `std::fs::read("")`, which fails → `ERR_OCR_IO_FAILED`, not InvalidImage (native.rs:173-175). The row's own "Covered by: none" is accurate | Partial |
| 17 | fromBytes → Encoded; Partial: no camera formats/rotation; undecodable bytes are the backend's INVALID_IMAGE (parity.md:46) | input.rs Encoded; row 14; ocrs decode unit test; other bridges compile only | Done |
| 18 | fromBitmap → Rgba, Partial: same pixels, no rotation (parity.md:47) | input.rs Rgba; rows 12-13 | Done |

### TextRecognitionScript

| # | Claim | Evidence | Status |
|---|---|---|---|
| 19-23 | The five scripts, names, primary languages; standard `devanagari` spelling; unknown name over the bridge is INVALID_ARGUMENT (parity.md:53-57) | options.rs:25-43 and options.ts match; Flutter's `devanagiri` confirmed at .refs text_recognizer.dart:41; native.rs:187-193 + unit test | Done |

### The react-native-mlkit-text-recognition module

| # | Claim | Evidence | Status |
|---|---|---|---|
| 24 | recognizeText(imagePath) → path variant, script per call (parity.md:63) | native.rs:171-175; row 1 | Done |
| 25 | Text/Block/TextLine/TextElement/Rect mapped with the two renames and extra fields (parity.md:64-68) | text.rs; row 21 | Done |
| 26 | close()/cancel() added (parity.md:69) | Rows 3, 10 both languages | Done |
| 27 | confidence/angle/symbols are Platform only (parity.md:70) | Per-platform filling verified by code read (apple: confidence no angle; android: both + symbols; windows/ocrs: neither); shape tested by row 21 | Done |

### Platform capabilities

| # | Claim | Evidence | Status |
|---|---|---|---|
| 28 | Apple Vision accurate level, language resolved before start, observation→line mapping (parity.md:76) | apple.rs:275-280 (check first), 110-153 (accurate request, one TextLine per observation, confidence Some, angle None); pure-mapping unit tests run on macOS; the Vision call itself untested | Partial |
| 29 | Apple cancel_recognition is a no-op; core drops the late answer (parity.md:77) | apple.rs:297; rows 5, 8 | Done |
| 30 | Android ML Kit v2 over JNI, one recognizer per script, decodes both input forms, exactly one answer (parity.md:78) | android.rs matches the whole design (options map android.rs:161-185, bitmap android.rs:458-505, listener arm android.rs:601-662). Compiles in cross-check only; its inline tests never run in CI; no device test | Partial |
| 31 | Android confidence/angle on lines and elements (parity.md:79) | android.rs:839-840, 859-860; no runner shows it | Partial |
| 32 | Android answers the five script languages (parity.md:80) | `supported()` android.rs:202-213; its unit test never runs in CI (Known gaps row 2) | Partial |
| 33 | Windows: TryCreateFromLanguage with IsLanguageSupported fallback, runtime language list (parity.md:81) | windows.rs:248-253, 181-189; engine path untested | Partial |
| 34 | Windows: no confidence/angle; cancel no-op (parity.md:82) | windows.rs:139-161 (None/None), 322-325 | Partial |
| 35 | Windows BGRA conversion is a pure unit-tested function (parity.md:83) | `rgba_to_bgra` windows.rs:55-61; test at windows.rs:333 runs on the Windows runner | Done |
| 36 | ocrs: from_model_bytes parses eagerly; use_ocrs_models sets process-wide bytes, later call replaces; without models processImage rejects MODEL_NOT_LOADED; supported_languages still ["en"] (parity.md:84) | ocrs.rs:86-128, 142-184; unit tests on Linux; row 20. True at the Rust level — but unreachable from TSX (Gap G1), so on Linux/web every `rustBackend()` recognition is MODEL_NOT_LOADED today | Partial |
| 37 | ocrs Latin only, refused before any work (parity.md:85) | ocrs.rs:66-72 + unit test; row 18 | Done |
| 38 | ocrs lines/words mapping, one block per image, no confidence (parity.md:86) | map_ocrs_line/build_recognized_text ocrs.rs:270-400; unit tests `build_recognized_text_*`; rows 21, 25 | Done |
| 39 | Any other target: start is NOT_AVAILABLE, supported_languages is empty (parity.md:87) | unsupported.rs:15-31; no test drives it (row 15 tests the plumbing) | Partial |

### Errors

| # | Claim | Evidence | Status |
|---|---|---|---|
| 40 | Corrupt/wrong-shape image → INVALID_IMAGE, core validates first (parity.md:95) | Rows 12-14; per-platform decode refusal only unit-tested for ocrs | Partial |
| 41 | Unreadable path → IO_FAILED from native.rs (parity.md:96) | native.rs:171-175 + unit test native.rs:325 | Done |
| 42 | Unsupported language/script → UNSUPPORTED_LANGUAGE from Apple/Windows/Linux/web (parity.md:97) | Row 18 plumbing; producers compile only (ocrs producer unit-tested on Linux) | Partial |
| 43 | Engine failure → BACKEND with the platform status where given (parity.md:98) | Row 19 + row 17 plumbing; producers compile only | Partial |
| 44 | Model problems → MODEL_NOT_LOADED on Linux/web (parity.md:99) | ocrs unit tests + row 20 | Done |
| 45 | Closed recognizer → CLOSED from the core (parity.md:100) | Row 9 both languages | Done |
| 46 | Cancel → CANCELLED from the core; late answer dropped (parity.md:101) | Rows 3, 11, 8 | Done |
| 47 | Malformed args → INVALID_ARGUMENT: zero timeout, unknown script over the bridge (parity.md:102) | Row 7; native.rs unit test native.rs:258 | Done |
| 48 | Timeout → TIMEOUT, Rust timer on non-wasm32, TS watchdog everywhere (parity.md:103) | Row 6 both languages | Done |
| 49 | No engine on the target → NOT_AVAILABLE (parity.md:104) | unsupported.rs; plumbing row 15 only | Partial |
| 50 | "The set is held together by" the three error tests (parity.md:106) | All three exist in both languages (errors.rs:103/117/133; errors.test.ts:72/83/90) | Done |

## docs/brief.md

| # | Claim | Evidence | Status |
|---|---|---|---|
| 1 | "ocrs: pure Rust; ships model files; pre-1.0 and breaking releases" (brief.md:8) | ocrs does not ship models — the app ships them (decisions.md:31 corrects this); pre-1.0 true (pin Cargo.toml:47) | Partial |
| 2 | "Vision on Apple … ocrs elsewhere, with pinned versions" (brief.md:13) | Superseded by decisions.md:28-31: Android runs ML Kit, Windows runs Media.Ocr; ocrs is Linux/web only | Stale |
| 3 | Edge cases: query supportedRecognitionLanguages; models ship with the app; pin versions (brief.md:17-19) | apple.rs:34-41; the crate takes bytes, never fetches (ocrs.rs:121); pins in Cargo.toml | Done |
| 4 | "Versions and licences are unverified until checked on the registry before pinning" (brief.md:23) | The pins were then verified (Cargo.toml comments name the checks; ocrs 0.14.0 and rten 0.28.0 resolved in Cargo.lock) | Done |

## docs/decisions.md

| # | Claim | Evidence | Status |
|---|---|---|---|
| 1 | Five references cloned into `.refs/`, read before code (decisions.md:5-13) | All five directories exist; spot-verified text_recognizer.dart:41, RNMLKitTextRecognitionModule.ts:36, ocrs/src/errors.rs, nsh-core/src/runtime/{fs,image}.rs; `.refs/` is git-ignored | Done |
| 2 | The RN reference is maintained, repo at v6.0.0, `recognizeText(imagePath)` shape (decisions.md:17-22) | Module version 6.0.0 in .refs package.json; API shape matches .refs source:36. The maintenance-history dates (pushed 2026-10-29, Expo SDK 56 merge, npm 5.0.1) are upstream facts this checkout cannot show (refs HEAD is 2026-08-05) — not verified here | Partial |
| 3 | Engines per platform; every other target NOT_AVAILABLE (decisions.md:28-31) | platform/mod.rs:28-46 dispatch matches exactly | Done |
| 4 | "NSH is pinned to 0.5.1" — nsh-core 0.5.1, CI installs v0.5.1, pin history (decisions.md:35-37) | Cargo.toml:17; ci.yml:86, 117; history matches git log (8f3dd4f 0.5.0, 3d55e31 0.5.1) | Done |
| 5 | Input is Encoded bytes or Rgba; path only on the native TSX class (decisions.md:41) | input.rs:6-16; native.rs:171-184 | Done |
| 6 | Nothing built on the machine; CI compiles and tests everything (decisions.md:45) | Green runs; same untracked-local-target note as README row 7 | Done |
| 7 | Demo APK is a Gradle shell: carries ML Kit + NativeCallback, merge script zips classes2.dex/lib/assets, re-signs; manifest replicates NSH's own so AppActivity starts; Latin model only (decisions.md:49-59) | examples/demo/android/* matches line by line: NativeCallback.java carries the documented class, merge-nsh-into-apk.sh does exactly the documented steps, manifest declares `com.longbridge.nsh.AppActivity` (AndroidManifest.xml:27), build.gradle:32 ships only text-recognition 16.0.1; ci.yml:99-155 runs it; artifact upload enforced | Done |

## AGENTS.md

| # | Claim | Evidence | Status |
|---|---|---|---|
| 1 | "The main branch is `master`" | git branch; pushes land on master | Done |
| 2 | "GitHub Actions compiles and tests everything" | Green CI runs since 2026-11-11 | Done |
| 3 | "Status claims in README and docs must match what CI runs" | The rule holds; README.md:9, goals.md:27, parity.md:12 and site/index.html:23 currently break it (rows above) | Done |

## .github/workflows/ci.yml

| # | Claim (the workflow's own behaviour) | Evidence | Status |
|---|---|---|---|
| 1 | format job: `cargo fmt --all -- --check` on ubuntu (ci.yml:23-32) | Present; green in 44000499013 | Done |
| 2 | rust job: clippy then test, `-p nsh-ocr --all-targets --all-features`, three OS, fail-fast off (ci.yml:34-49) | Present; green on all three OS | Done |
| 3 | cross-check: the four targets with the NDK env block (ci.yml:51-77) | Present; green | Done |
| 4 | typescript job: NSH v0.5.1, check/build-warning-free/test, in `packages/nsh-ocr` (ci.yml:79-97) | Present; green. It covers `packages/nsh-ocr` only — no `nsh check/build/test` for `examples/demo` on master (see Gap G3) | Done |
| 5 | demo job: Android only, arm64, uploads `demo-android` (ci.yml:99-155) | Present; green | Done |
| 6 | NSH installed from `raw.githubusercontent.com/SuperNSH/nsh-builds/master/install.sh` (ci.yml:88, 119) | Installs fine (green runs). The URL is the biometrics-era one (.refs/biometrics ci.yml:116); the official installer today is https://nsh.dev/install.sh (also live). Works, but the older source | Partial |
| 7 | Header: "Every compile and test runs here, on GitHub's runners, never on a developer's machine" (ci.yml:3) | True of CI; untracked local target/ dirs exist anyway (parallel work) | Done |

## .github/workflows/pages.yml

| # | Claim | Evidence | Status |
|---|---|---|---|
| 1 | "Publishes the static status page in site/ to GitHub Pages on every push to master" (pages.yml:3) | Pages runs green through 2026-11-17; URL answers 200 | Done |

## site/index.html

| # | Claim | Evidence | Status |
|---|---|---|---|
| 1 | "**Status: scaffold.** No implementation yet." (site/index.html:23) | The crate and package are implemented and CI is green — the page is behind by six days of runs | Stale |
| 2 | "This page is a status page, not a demo." (site/index.html:23) | True: static HTML, no demo | Done |

## .zcode/workflow.ts (planning artifact)

| # | Claim | Evidence | Status |
|---|---|---|---|
| 1 | Gates: `cargo clippy --all-targets -- -D warnings` etc. (workflow.ts:16-19) | Superseded: real CI gates are `-p nsh-ocr --all-features` (ci.yml:48, 77) plus the TS and demo jobs | Stale |
| 2 | area "ocr" — every phase "not-started" (workflow.ts:24-31) | Implement and unit-tests are done and CI-green; audit is this document; device-test/vision/smoke are not (see Gap G3/G5) | Stale |
| 3 | Phase list: implement, audit, unit-tests, device-test, vision, smoke (workflow.ts:5) | Three of six done; device-test is only the manual Android APK artifact; vision/smoke never ran | Partial |

## .zcode/workflow-drafts/spec/contract.md (planning artifact; docs/ + code are canonical)

| # | Claim | Evidence | Status |
|---|---|---|---|
| 1 | Workspace manifest values and crate layout (contract.md:18-28) | Cargo.toml matches every value (version 0.1.0, edition 2024, MIT, repo, hmziqrs) | Done |
| 2 | `nsh-core = "0.4.0" (exact, this is the NSH pin)` (contract.md:30) | Superseded by decisions.md:35-37 and Cargo.toml:17 (0.5.1) | Stale |
| 3 | Target-gated dependency set (contract.md:31-36) | All as specified: thiserror 2, tokio sync+time (Cargo.toml:19-21); objc2 family on apple (25-31); jni 0.22 + ndk-context (34-36); windows 0.62 + windows-future std (40-42); ocrs 0.14/rten 0.28/image 0.25 png+jpeg (45-51) | Done |
| 4 | Exact module tree and re-export list incl. cfg-gated ocrs re-exports (contract.md:37-52) | Every file exists; lib.rs:19-39 re-exports exactly that list, `use_ocrs_models`/`OcrsBackend` gated to linux/wasm32 | Done |
| 5 | Ten frozen codes and messages (contract.md:57-67) | error.rs:74-101 identical; unit test pins it | Done |
| 6 | ImageInput enum + validate semantics (contract.md:71-77) | input.rs:6-43 | Done |
| 7 | Script enum (variant `Devanagiri`, the standard name string), options (contract.md:80-85) | options.rs:9-58 | Done |
| 8 | Result structs snake_case here, camelCase in TSX (contract.md:88-95) | text.rs vs text.ts field for field | Done |
| 9 | Backend trait + BackendFuture + RecognitionDone (contract.md:98-107) | backend.rs:11-36 verbatim | Done |
| 10 | FakeBackend surface, frozen names (contract.md:110-120) | fake.rs:44-110 — every method with that signature | Done |
| 11 | Core semantics: validate → options check → closed check → register → start → wait; wasm32 timer compiled out; drop cancels (contract.md:122-131) | recognizer.rs:88-231 matches in order and in every branch | Done |
| 12 | Native TSX class surface and error mapping (contract.md:133-143) | native.rs matches; JsError carries message + code (native.rs:152-154, unit test at 409) | Done |
| 13 | apple.rs design (contract.md:146) | Implemented as written (runtime query before start, pure mappings, cancel no-op) | Done |
| 14 | android.rs design incl. no-panic/no-hang JNI rules (contract.md:147) | Implemented (SAFETY comments on every unsafe block; catch_unwind on the listener path) | Done |
| 15 | windows.rs design (contract.md:148) | Implemented (RGBA→BGRA pure+tested, decoder chain, language check before pixel work) | Done |
| 16 | ocrs.rs design (contract.md:149-153) | Implemented (eager parse, process-wide bytes, lazy engine, Latin refusal, one block) | Done |
| 17 | TS package: files, exports, error wording, resolve-once machinery, memory double, rustBackend (contract.md:156-169) | All present; package.json byte-matches the contract's JSON; wording identical | Done |
| 18 | CI: typescript job installs NSH v0.4.0; "No emulator, no device job, no demo packaging job" (contract.md:175-176) | Superseded: v0.5.1 (ci.yml:86) and the demo packaging job exists, documented in decisions.md:49-59 | Stale |
| 19 | Test style: vitest, `it()` phrases, `vi.useFakeTimers()` for the timeout row (contract.md:180) | Superseded: NSH's own `nsh/test` harness with `test()`; the timeout row uses real timers | Stale |
| 20 | The 26 frozen names in both languages (contract.md:182-209) | All 52 exist with exact names (goals matrix above) | Done |

## .zcode/workflow-drafts/spec/ — the briefs (planning artifacts)

| # | Claim | Evidence | Status |
|---|---|---|---|
| preamble.md: rustfmt the only allowed local command; no builds/tests locally | This audit ran read-only; CI carries the proof | Done |
| audit_preamble.md: audits are read-only, findings-only | Followed by this document | Done |
| brief-core.md: own the core files, module tree, delete the scaffold's root `src/` | No root `src/` exists; the tree matches | Done |
| brief-core.md:4 "nsh-core is `0.4.0` exactly" | Superseded (0.5.1) | Stale |
| brief-core.md: inline unit tests for pure functions (code/message tables, validate, script names, native mapping) | All four exist (error.rs:169-202, input.rs:50-117, options.rs:60-90, native.rs:253-420) | Done |
| brief-goals.md: goals.md structure (goals, acceptance criteria, ten-error table, frozen matrix, Known gaps heading) | goals.md has exactly that shape | Done |
| brief-parity.md: parity.md style/tables/intro sentence; README updated to "implemented, awaiting CI" | Followed; the README line has since gone stale (CI green) — README row 2 | Done |
| brief-typescript.md: every file/name, resolve-once machinery, no `any`, casts only at the native boundary | grep finds no `any`; the single cast is rust.ts:40 with a comment | Done |
| brief-bridge-apple.md: pure mappings with unit tests free of Vision types | apple.rs:158-257 pure + tests 304-427 | Done |
| brief-bridge-android.md: signatures/mappings in pure unit-tested helpers; SAFETY comments | android.rs:104-223 pure helpers + tests 1003-1129 (which CI never runs — Known gaps row 2); SAFETY comments throughout | Done |
| brief-bridge-windows.md: RGBA→BGRA pure unit-tested; mapping free of windows types | windows.rs:43-178 + tests 328-520 | Done |
| brief-bridge-ocrs.md: unit tests for pure mappings and model-less paths, never load a real model | ocrs.rs:402-654; no model file is ever loaded | Done |
| brief-tests-ci.md: three Rust files with the matrix, common/mod.rs, run()/settle() helpers | tests/common/mod.rs:8-25 exactly that | Done |
| brief-tests-ci.md:4 row 6 uses a real short sleep and proves the late answer dropped; row 8 proves `cancels()` increments | recognizer.rs:109-124 and 148-169 | Done |
| brief-tests-ci.md:5 TS timeout row "driven with `vi.useFakeTimers()`" | Superseded: real 25 ms timeout + `waited(50)` (recognizer.test.ts:121-138) | Stale |
| brief-tests-ci.md:6 CI mirrors the contract incl. NSH v0.4.0 install | Superseded on the version (0.5.1) and exceeded by the demo job | Stale |

## Gaps this audit was asked to row

| # | Gap | Evidence on master | Status |
|---|---|---|---|
| G1 | `use_ocrs_models` is exported on Linux/wasm32 (lib.rs:38-39, ocrs.rs:121) but nothing bridges it to TSX: the native module registers only `NativeTextRecognizer` (lib.rs:32-35), and `packages/nsh-ocr/src/rust.ts` wraps only that class. A TS app on Linux/web can never load models, so every recognition is `ERR_OCR_MODEL_NOT_LOADED` (ocrs.rs:149-151) | grep over packages/nsh-ocr finds no reference to the models API | Missing |
| G2 | "rten is built with only the rten_format feature, so even Rust callers cannot load the public .onnx models" — this premise does not hold on master. The direct pin is rten_format-only (Cargo.toml:48-49), but `ocrs = "0.14.0"` keeps default features, whose `onnx` feature enables `rten/onnx_format` (ocrs-0.14.0 Cargo.toml `[features]`); feature unification turns ONNX loading on, and rten 0.28's `Model::load` auto-detects and loads `.onnx` under that feature (rten-0.28.0/src/model.rs:807-824). S3 hosts only `.onnx` (curl: 200) and no `.rten` (curl: 403). So Rust callers CAN load the public models. What is actually wrong: the Cargo.toml comment ("rten_format loads the .model bytes") suggests `.onnx` is unsupported, and correctness silently depends on ocrs's default features | Registry sources read at the pinned versions; S3 headers checked | Stale |
| G3 | No `nsh check/build/test` for `examples/demo` and no demo UI test in CI on master (the typescript job covers `packages/nsh-ocr` only, ci.yml:92) | ci.yml read in full | Missing |
| G4 | No GitHub release exists (`gh release list` is empty); nothing serves a `latest` artifact | Read-only gh call | Missing |
| G5 | The demo job builds Android only (ci.yml:99-155); no iOS/desktop/web demo packaging, no device or emulator job | ci.yml read in full | Partial |
| G6 | README promises only a "web status page" on Pages — that part is true today (README row 1); no doc promises a web demo, and none exists | pages.yml + curl | Done |

## NSH version sweep

Every mention of 0.4.0/0.5.0/0.5.1 behaviour in docs/ and code comments on master, and its verdict. The repo is on 0.5.1, a bug-fix release over 0.5.0.

| Where | What it says | Verdict |
|---|---|---|
| docs/decisions.md:11 | "`nsh-core` 0.5.1 gives `#[class]`, `#[derive(JsObject)]`, …" | Current — keep |
| docs/decisions.md:37 | Pin history: 0.4.0 → 0.5.0 (2026-11-12) → 0.5.1 (2026-11-17) | Matches git log — keep |
| docs/decisions.md:49 | "NSH 0.5.0 writes the Android APK itself and offers no way to add Java code" | Still true as of 0.5.1 (bug-fix only) — keep, say "as of 0.5.1" |
| crates/.../platform/android.rs:33 | "NSH 0.5.0 writes the Android packaging itself and offers no way to add Java" | Same — keep as of 0.5.1 |
| examples/demo/android/app/src/main/AndroidManifest.xml:2 | "NSH 0.5.0's own Android manifest (nsh-cli/src/android.rs manifest_xml), replicated" | Replica matches the load-bearing parts (AppActivity, FilesProvider, RUST meta-data, INTERNET). NSH main's manifest adds POST_NOTIFICATIONS only when notifications are on (.refs/nsh/crates/nsh-cli/src/android.rs:367-369, 394) and the demo requests none — keep as of 0.5.1 |
| docs/goals.md:18 | TypeScript gate "under NSH v0.5.1" | Current — keep |
| crates/nsh-ocr/Cargo.toml:16-17 | "Exactly 0.5.1" | Current — keep |
| .github/workflows/ci.yml:86,117 | `NSH_VERSION: v0.5.1` | Current — keep |
| .zcode spec files (contract.md:30,175; brief-core.md:4; brief-tests-ci.md:6; nsh-ocr-build.ts:50,195,285,359,817) | nsh-core "0.4.0" / NSH v0.4.0 install | Stale — superseded by decisions.md:35-37 and the code |
| ci.yml:88,119 | Installer URL `SuperNSH/nsh-builds/master/install.sh` | Works (green runs), but it is the biometrics-era URL; the official installer is https://nsh.dev/install.sh (both live). Worth switching |

## What could not be verified

- decisions.md:17-22's upstream-history claims (repo pushed 2026-10-29, Expo SDK 56 merge, npm latest 5.0.1 published 2025-11-25, the rejected libraries' archival dates). The `.refs` checkout's API shape and version 6.0.0 are verified; the dates are not checkable from this repo.
- Whether the four platform bridges work at runtime. CI compiles them (cross-check, green) but no runner holds an engine, model or device — goals.md:85 says so, and this audit confirms it.
