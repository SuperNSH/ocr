/** A box in pixels, top-left origin. */
export interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** A corner of a box, in pixels. */
export interface Point {
  x: number;
  y: number;
}

/** All the text an image held. `text` joins every block's text. */
export interface RecognizedText {
  text: string;
  blocks: TextBlock[];
}

/** A paragraph of text: its lines and where they sit. */
export interface TextBlock {
  text: string;
  lines: TextLine[];
  boundingBox: Rect;
  languages: string[];
  cornerPoints: Point[];
}

/** A line of text: its elements and where it sits. */
export interface TextLine {
  text: string;
  elements: TextElement[];
  boundingBox: Rect;
  languages: string[];
  cornerPoints: Point[];
  /** `null` where the platform gives no confidence. */
  confidence: number | null;
  /** `null` where the platform gives no angle. */
  angle: number | null;
}

/** A word of text: its symbols and where it sits. */
export interface TextElement {
  text: string;
  symbols: TextSymbol[];
  boundingBox: Rect;
  languages: string[];
  cornerPoints: Point[];
  /** `null` where the platform gives no confidence. */
  confidence: number | null;
  /** `null` where the platform gives no angle. */
  angle: number | null;
}

/** A single character and where it sits. */
export interface TextSymbol {
  text: string;
  boundingBox: Rect;
  languages: string[];
  cornerPoints: Point[];
  /** `null` where the platform gives no confidence. */
  confidence: number | null;
  /** `null` where the platform gives no angle. */
  angle: number | null;
}
