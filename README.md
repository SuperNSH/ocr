# nsh-ocr

Recognise text in images.

Part of the Native Shell (NSH) capability set. Same layout as the other SuperNSH repos: a Rust crate and a TypeScript package, docs in `docs/`, CI on GitHub Actions. The names follow Flutter's [`google_mlkit_text_recognition`](https://pub.dev/packages/google_mlkit_text_recognition) and the React Native module [`react-native-mlkit-text-recognition`](https://www.npmjs.com/package/@infinitered/react-native-mlkit-text-recognition), so an app that knows either knows this one; `docs/parity.md` maps every reference API to its Rust and TSX counterpart.

## Status

Implemented. The crate in `crates/nsh-ocr/` and the package in `packages/nsh-ocr/` are written against the frozen contract: ten coded errors, image input as a file path, encoded bytes or RGBA pixels, five scripts, a recognizer whose calls resolve exactly once, and bridges to Apple Vision, Android ML Kit, `Windows.Media.Ocr` and ocrs. All gates run in CI and the tests are green on `master`'s runs. The bridges compile on every target, but no CI runner holds an engine or a device, so recognition on a device or desktop is checked by hand through the demo builds below — and the web demo runs real recognition in the browser. Read [docs/brief.md](docs/brief.md) for the approach and [docs/parity.md](docs/parity.md) for the row-by-row status.

## Engines

Vision on Apple (macOS, iOS), its supported languages queried at runtime. ML Kit text recognition v2 on Android, one recognizer per script. `Windows.Media.Ocr` on Windows, its language list queried at runtime. ocrs on Linux and the web (wasm32), Latin only, with model files the app supplies. Every other target answers `ERR_OCR_NOT_AVAILABLE`.

## Try the demo

The demo in `examples/demo` shows the engine's supported languages, the ocrs model files loading on Linux and the web, and recognition of a rendered HELLO OCR bitmap. Open it on the web at https://supernsh.github.io/ocr/ (the web runs ocrs with the bundled models, so recognition there is real), or download a build of the latest `master`:

| Platform | Download |
|---|---|
| macOS (Apple silicon) | [ocr-demo-macos-arm64.zip](https://github.com/SuperNSH/ocr/releases/latest/download/ocr-demo-macos-arm64.zip) |
| macOS (Intel) | [ocr-demo-macos-x86_64.zip](https://github.com/SuperNSH/ocr/releases/latest/download/ocr-demo-macos-x86_64.zip) |
| Linux x86_64 | [ocr-demo-linux-x86_64.tar.gz](https://github.com/SuperNSH/ocr/releases/latest/download/ocr-demo-linux-x86_64.tar.gz) |
| Linux arm64 | [ocr-demo-linux-arm64.tar.gz](https://github.com/SuperNSH/ocr/releases/latest/download/ocr-demo-linux-arm64.tar.gz) |
| Windows x86_64 | [ocr-demo-windows-x86_64.zip](https://github.com/SuperNSH/ocr/releases/latest/download/ocr-demo-windows-x86_64.zip) |
| Windows arm64 | [ocr-demo-windows-arm64.zip](https://github.com/SuperNSH/ocr/releases/latest/download/ocr-demo-windows-arm64.zip) |
| iOS simulator | [ocr-demo-ios-simulator.zip](https://github.com/SuperNSH/ocr/releases/latest/download/ocr-demo-ios-simulator.zip) |
| Android (arm64) | [ocr-demo-android.apk](https://github.com/SuperNSH/ocr/releases/latest/download/ocr-demo-android.apk) |
| Web (static site) | [ocr-demo-web.tar.gz](https://github.com/SuperNSH/ocr/releases/latest/download/ocr-demo-web.tar.gz) |

- **macOS:** the app is signed ad hoc, not notarized, and needs macOS 14 or later. macOS quarantines what a browser downloads, so clear that before opening it: `xattr -dr com.apple.quarantine <the unzipped .app>`.
- **iOS simulator:** an arm64 simulator build, so it needs Xcode with an iOS simulator runtime on an Apple silicon Mac. Unzip it and install it on a booted simulator with `xcrun simctl install booted <the unzipped .app>`, or drag the `.app` onto the simulator window.
- **Android:** the APK carries ML Kit's Latin recognizer only. The other four scripts' recognizers are not bundled, so they answer `ERR_OCR_UNSUPPORTED_LANGUAGE` on the device.

## Layout

- `crates/nsh-ocr/`: the Rust crate: errors, input, options, result types, the backend trait, the recognizer core, the native class and the platform bridges.
- `packages/nsh-ocr/`: the TypeScript package: the same API in TSX names, a memory backend for tests, and the wrapper over the native class.
- `examples/demo/`: the demo app. `fetch-models.sh` fetches the ocrs models it bundles; `android/` is the Gradle shell behind its APK; `tests/` drives the screen under `nsh test`.
- `docs/`: `brief.md` (options and path from the research), `decisions.md` (reference and engine decisions), `goals.md` (goals and the error-code catalogue), `parity.md` (status against the references).
- `.github/workflows/ci.yml`: every gate, plus the demo packaging, the `latest` release and the Pages deploy.

## Checks

Every compile and test runs in GitHub Actions. Nothing is built on a developer machine. CI (`.github/workflows/ci.yml`) runs:

- `cargo fmt --all -- --check`.
- Clippy (`-D warnings`) and the test suite on Linux, macOS and Windows.
- A cross-target Clippy check for `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `aarch64-linux-android` and `wasm32-unknown-unknown`, each with `--all-targets`, so every bridge's inline unit tests are parsed on their own target.
- The TypeScript package and the demo under NSH v0.5.1: `nsh check`, a warning-free `nsh build`, `nsh test`.

On `master`, after those pass, CI packages the demo for nine targets: macOS, Linux and Windows on both architectures, the iOS simulator, Android, and the web. The Android build goes through a Gradle shell (`examples/demo/android/`) that merges ML Kit and the JNI callback class into NSH's APK; the other eight build straight from `nsh package`. The nine builds replace the `latest` release (the downloads in [Try the demo](#try-the-demo)), and the web build is deployed to GitHub Pages at https://supernsh.github.io/ocr/.

The demo bundles the ocrs model files (`text-detection.onnx`, `text-recognition.onnx`) as app assets: the only public copies sit on S3 without CORS headers, so a browser cannot fetch them at runtime. `examples/demo/fetch-models.sh` downloads them, pinned by sha256 (the S3 URLs are not guaranteed stable), into `examples/demo/assets/`; the files are gitignored, so a fresh checkout runs the script once before building or testing the demo. CI does that in every job that touches the demo.

## Licence

MIT. See [LICENSE](LICENSE).

Repository: https://github.com/SuperNSH/ocr
