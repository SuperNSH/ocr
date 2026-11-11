# nsh-ocr

Recognise text in images.

Part of the Native Shell (NSH) capability set. Same layout as the other SuperNSH repos: a Rust crate and a TypeScript package, docs in `docs/`, CI on GitHub Actions, and a web status page on GitHub Pages at https://supernsh.github.io/ocr/.

## Status

**Implemented, awaiting CI.** The crate in `crates/nsh-ocr/` and the package in `packages/nsh-ocr/` are written against the frozen contract: ten coded errors, image input as encoded bytes or RGBA pixels, five scripts, a recognizer whose calls resolve exactly once, and bridges to Apple Vision, Android ML Kit, Windows.Media.Ocr and ocrs. No CI run has compiled or tested any of it yet, so no behaviour is verified. Read [docs/brief.md](docs/brief.md) for the approach and [docs/parity.md](docs/parity.md) for the row-by-row status.

## Planned approach

Vision on Apple, queried at runtime for supported languages. ocrs elsewhere, with pinned versions.

## Layout

- `crates/nsh-ocr/`: the Rust crate: errors, input, options, result types, the backend trait, the recognizer core, the native class and the platform bridges.
- `packages/nsh-ocr/`: the TypeScript package: the same API in TSX names, a memory backend for tests, and the wrapper over the native class.
- `examples/demo/`: the demo app, packaged by CI into an installable Android APK.
- `docs/`: `brief.md` (options and path from the research), `decisions.md` (reference and engine decisions), `goals.md` (goals and the error-code catalogue), `parity.md` (status against the references).
- `site/`: static status page, published by `.github/workflows/pages.yml`.
- `.github/workflows/`: CI (`ci.yml`) and the Pages publish.

## Checks

Every compile and test runs in GitHub Actions. Nothing is built on a developer machine.

CI also packages `examples/demo` for Android: NSH builds the app, a Gradle shell carries ML Kit and the JNI callback class, and the merged APK is uploaded as the run's `demo-android` artifact (arm64, Latin model only — the other scripts answer a coded error on the device). Install it from the run's artifact page to check recognition on a real device.

## Licence

MIT. See [LICENSE](LICENSE).

Repository: https://github.com/SuperNSH/ocr
