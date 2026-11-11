//! What a recognition gives back: the text, and the tree it was read from.
//!
//! [`Rect`] and [`Point`] derive `JsObject` because the native result types carry them as they
//! are; field names are snake_case here and camelCase in TSX, which `JsObject` applies.

use nsh_core::element::JsObject;

/// An axis-aligned box in pixel space, as edges from the top-left of the image.
#[derive(JsObject, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    /// The left edge's x.
    pub left: i32,
    /// The top edge's y.
    pub top: i32,
    /// The right edge's x.
    pub right: i32,
    /// The bottom edge's y.
    pub bottom: i32,
}

/// A pixel-space point, from the top-left of the image.
#[derive(JsObject, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    /// The point's x.
    pub x: i32,
    /// The point's y.
    pub y: i32,
}

/// One recognition's whole answer: the joined text and the blocks it was read from.
///
/// An image with no text resolves `Ok` with empty text and no blocks; that is success, not an error.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RecognizedText {
    /// Every line's text, joined with newlines.
    pub text: String,
    /// The blocks of text the platform laid out, empty when it has no block concept.
    pub blocks: Vec<TextBlock>,
}

/// A paragraph-sized group of lines, as the platforms that have blocks report it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextBlock {
    /// The block's own text: its lines joined.
    pub text: String,
    /// The lines of the block, top to bottom.
    pub lines: Vec<TextLine>,
    /// The box every line of the block sits in.
    pub bounding_box: Rect,
    /// The BCP-47 languages the recognizer read the block as.
    pub languages: Vec<String>,
    /// The block's corners, clockwise from the top-left.
    pub corner_points: Vec<Point>,
}

/// One line of text. [`TextLine::confidence`] and [`TextLine::angle`] are `Some` only where the
/// platform provides them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextLine {
    /// The line's own text: its elements joined.
    pub text: String,
    /// The words of the line, left to right.
    pub elements: Vec<TextElement>,
    /// The box the line sits in.
    pub bounding_box: Rect,
    /// The BCP-47 languages the recognizer read the line as.
    pub languages: Vec<String>,
    /// The line's corners, clockwise from the top-left.
    pub corner_points: Vec<Point>,
    /// How sure the recognizer is, from 0 to 1; Android and Apple give one.
    pub confidence: Option<f32>,
    /// The line's rotation in degrees; only Android gives one.
    pub angle: Option<f32>,
}

/// One word of a line. [`TextElement::confidence`] and [`TextElement::angle`] are `Some` only
/// where the platform provides them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextElement {
    /// The element's own text: its symbols joined.
    pub text: String,
    /// The characters of the element, or empty where the platform has none.
    pub symbols: Vec<TextSymbol>,
    /// The box the element sits in.
    pub bounding_box: Rect,
    /// The BCP-47 languages the recognizer read the element as.
    pub languages: Vec<String>,
    /// The element's corners, clockwise from the top-left.
    pub corner_points: Vec<Point>,
    /// How sure the recognizer is, from 0 to 1; Android and Apple give one.
    pub confidence: Option<f32>,
    /// The element's rotation in degrees; only Android gives one.
    pub angle: Option<f32>,
}

/// One character of an element. [`TextSymbol::confidence`] and [`TextSymbol::angle`] are `Some`
/// only where the platform provides them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextSymbol {
    /// The character itself.
    pub text: String,
    /// The box the symbol sits in.
    pub bounding_box: Rect,
    /// The BCP-47 languages the recognizer read the symbol as.
    pub languages: Vec<String>,
    /// The symbol's corners, clockwise from the top-left.
    pub corner_points: Vec<Point>,
    /// How sure the recognizer is, from 0 to 1; only Android gives one.
    pub confidence: Option<f32>,
    /// The symbol's rotation in degrees; only Android gives one.
    pub angle: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tree_carries_every_level_and_the_optional_values_stay_distinct() {
        let symbol = TextSymbol {
            text: "a".to_owned(),
            bounding_box: Rect {
                left: 0,
                top: 1,
                right: 2,
                bottom: 3,
            },
            languages: vec!["en-US".to_owned()],
            corner_points: vec![Point { x: 0, y: 1 }],
            confidence: Some(0.5),
            angle: None,
        };
        let element = TextElement {
            text: "a".to_owned(),
            symbols: vec![symbol.clone()],
            ..TextElement::default()
        };
        let line = TextLine {
            text: "a".to_owned(),
            elements: vec![element],
            confidence: None,
            angle: Some(2.5),
            ..TextLine::default()
        };
        let block = TextBlock {
            text: "a".to_owned(),
            lines: vec![line],
            ..TextBlock::default()
        };
        let text = RecognizedText {
            text: "a".to_owned(),
            blocks: vec![block],
        };
        let symbol = &text.blocks[0].lines[0].elements[0].symbols[0];
        assert_eq!(symbol.text, "a");
        assert_eq!(symbol.bounding_box.bottom, 3);
        assert_eq!(symbol.confidence, Some(0.5));
        assert_eq!(symbol.angle, None);
        assert_eq!(text.blocks[0].lines[0].angle, Some(2.5));
        assert_eq!(
            text,
            text.clone(),
            "the tree compares by value, so a test can assert on it"
        );
    }

    #[test]
    fn an_empty_answer_is_the_default() {
        let empty = RecognizedText::default();
        assert!(empty.text.is_empty());
        assert!(empty.blocks.is_empty());
    }
}
