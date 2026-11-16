//! Word and character error rates; characters suit languages written without spaces.
//! Both ignore case, punctuation and spacing.

use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ErrorCount {
    /// Units (words or characters) in the reference.
    pub reference: usize,
    pub errors: usize,
}

impl ErrorCount {
    /// Can exceed 1 when much was inserted. `None` when the reference is empty.
    pub fn rate(&self) -> Option<f64> {
        (self.reference > 0).then(|| self.errors as f64 / self.reference as f64)
    }

    pub fn plus(&self, other: &Self) -> Self {
        Self {
            reference: self.reference.saturating_add(other.reference),
            errors: self.errors.saturating_add(other.errors),
        }
    }
}

/// Lower-cased; non-alphanumerics split words. Works on graphemes so combining marks such as
/// separate vowel signs stay with their letter.
pub fn comparable_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    for grapheme in text.graphemes(true) {
        if grapheme.chars().next().is_some_and(char::is_alphanumeric) {
            word.push_str(&grapheme.to_lowercase());
        } else if !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

fn edit_distance<T: PartialEq>(said: &[T], heard: &[T]) -> usize {
    let mut row: Vec<usize> = (0..=heard.len()).collect();
    for (i, wanted) in said.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, got) in heard.iter().enumerate() {
            let replaced = diagonal + usize::from(wanted != got);
            diagonal = row[j + 1];
            row[j + 1] = replaced.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    row[heard.len()]
}

pub fn word_errors(reference: &str, recognised: &str) -> ErrorCount {
    let said = comparable_words(reference);
    let heard = comparable_words(recognised);
    ErrorCount {
        reference: said.len(),
        errors: edit_distance(&said, &heard),
    }
}

/// Ignores where words are split.
pub fn character_errors(reference: &str, recognised: &str) -> ErrorCount {
    let chars = |text: &str| -> Vec<char> { comparable_words(text).concat().chars().collect() };
    let said = chars(reference);
    let heard = chars(recognised);
    ErrorCount {
        reference: said.len(),
        errors: edit_distance(&said, &heard),
    }
}
