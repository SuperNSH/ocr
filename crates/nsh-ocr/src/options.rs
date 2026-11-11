//! The script of text to look for, and the options of one recognition.

/// The script of text to recognise, as Flutter's `TextRecognitionScript` names it.
///
/// Flutter spells the third `devanagiri`; this is the standard spelling of the enum's name,
/// while [`Script::name`] keeps the TSX string `"devanagari"`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Script {
    /// Latin script, with the stock recognizers' English models.
    #[default]
    Latin,
    /// Simplified Chinese.
    Chinese,
    /// Devanagari script.
    Devanagiri,
    /// Japanese, kanji included.
    Japanese,
    /// Korean, hangul included.
    Korean,
}

impl Script {
    /// The name TSX uses: `"latin"`, `"chinese"`, `"devanagari"`, `"japanese"` or `"korean"`.
    pub fn name(self) -> &'static str {
        match self {
            Script::Latin => "latin",
            Script::Chinese => "chinese",
            Script::Devanagiri => "devanagari",
            Script::Japanese => "japanese",
            Script::Korean => "korean",
        }
    }

    /// The BCP-47 language this script's recognizers are asked for first.
    pub fn primary_language(self) -> &'static str {
        match self {
            Script::Latin => "en-US",
            Script::Chinese => "zh-Hans",
            Script::Devanagiri => "hi-IN",
            Script::Japanese => "ja-JP",
            Script::Korean => "ko-KR",
        }
    }
}

/// The options of one recognition. Start from [`RecognitionOptions::default`] and set what you need.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecognitionOptions {
    /// The script of text to look for. Defaults to Latin.
    pub script: Script,
    /// The longest the call waits, in milliseconds, before it resolves with
    /// [`ErrorKind::Timeout`]. Opt-in and off by default; on wasm32 it is accepted and unused
    /// (no time driver there), and the TypeScript layer runs its own watchdog.
    /// `Some(0)` is refused before any work starts.
    pub timeout_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_script_has_the_name_and_language_tsx_uses() {
        let table = [
            (Script::Latin, "latin", "en-US"),
            (Script::Chinese, "chinese", "zh-Hans"),
            (Script::Devanagiri, "devanagari", "hi-IN"),
            (Script::Japanese, "japanese", "ja-JP"),
            (Script::Korean, "korean", "ko-KR"),
        ];
        for (script, name, language) in table {
            assert_eq!(script.name(), name);
            assert_eq!(script.primary_language(), language);
        }
    }

    #[test]
    fn default_options_are_latin_without_a_timeout() {
        assert_eq!(Script::default(), Script::Latin);
        assert_eq!(
            RecognitionOptions::default(),
            RecognitionOptions {
                script: Script::Latin,
                timeout_ms: None,
            }
        );
    }
}
