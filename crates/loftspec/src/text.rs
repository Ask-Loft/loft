//! The tokenizer the Format's statement parser and language rules share: a text splits on
//! Unicode word bounds, and each segment holding a letter or a decimal digit is a token, in
//! lower case. Punctuation, spaces and symbols such as backticks are not tokens.

use std::ops::Range;

use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};
use unicode_segmentation::UnicodeSegmentation;

/// The tokens of `text`, lower-cased, in order.
#[must_use]
pub fn tokenize(text: &str) -> Vec<String> {
    token_ranges(text)
        .into_iter()
        .map(|range| lowercase(&text[range]))
        .collect()
}

/// A token in lower case.
pub(crate) fn lowercase(token: &str) -> String {
    token.chars().flat_map(char::to_lowercase).collect()
}

/// The byte range in `text` of each token [`tokenize`] returns, in the same order.
#[must_use]
pub fn token_ranges(text: &str) -> Vec<Range<usize>> {
    text.split_word_bound_indices()
        .filter(|(_, segment)| segment.chars().any(is_token_character))
        .map(|(start, segment)| start..start + segment.len())
        .collect()
}

fn is_token_character(character: char) -> bool {
    matches!(
        character.general_category(),
        GeneralCategory::UppercaseLetter
            | GeneralCategory::LowercaseLetter
            | GeneralCategory::TitlecaseLetter
            | GeneralCategory::ModifierLetter
            | GeneralCategory::OtherLetter
            | GeneralCategory::DecimalNumber
    )
}

#[cfg(test)]
mod tests {
    use super::{token_ranges, tokenize};

    #[test]
    fn tokens_are_lower_case_words_and_numbers_without_punctuation() {
        assert_eq!(
            tokenize("If `spec/` exists, then `loft init` shall exit 1."),
            [
                "if", "spec", "exists", "then", "loft", "init", "shall", "exit", "1"
            ]
        );
    }

    #[test]
    fn each_range_covers_its_token() {
        let text = "When a, b hold, the Publisher shall refuse.";
        let ranges = token_ranges(text);
        let tokens = tokenize(text);
        assert_eq!(ranges.len(), tokens.len());
        for (range, token) in ranges.iter().zip(&tokens) {
            assert_eq!(text[range.clone()].to_lowercase(), *token);
        }
    }
}
