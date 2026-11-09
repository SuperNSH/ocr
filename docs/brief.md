# OCR: brief

Recognise text in images.

## Options

- Vision VNRecognizeTextRequest: Apple, native
- ocrs: pure Rust; ships model files; pre-1.0 and breaking releases
- ML Kit text recognition: Android, reference

## Recommended path

Vision on Apple, queried at runtime for supported languages. ocrs elsewhere, with pinned versions.

## Edge cases to design for

- Language packs vary by OS version; query supportedRecognitionLanguages
- Model files must ship with the app
- ocrs releases break APIs; pin versions

## Research status

Research done 2026-11-09 by web search. Versions and licences are unverified until checked on the registry before pinning. Full catalogue: the research notes for the SuperNSH library set.
