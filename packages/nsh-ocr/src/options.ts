/** A script a recognizer reads, as TSX names it; Flutter spells the third one "devanagiri". */
export type Script = "latin" | "chinese" | "devanagari" | "japanese" | "korean";

/** Each script's primary language, the BCP-47 tag the platform recognizers take. */
export const SCRIPT_LANGUAGES: Record<Script, string> = {
  latin: "en-US",
  chinese: "zh-Hans",
  devanagari: "hi-IN",
  japanese: "ja-JP",
  korean: "ko-KR",
};

/** The options of one recognition. */
export interface RecognitionOptions {
  /** The script to read. Defaults to `latin`. */
  script?: Script;
  /** Milliseconds until the call gives up with `ERR_OCR_TIMEOUT`. Undefined waits for the platform. */
  timeoutMs?: number;
}
