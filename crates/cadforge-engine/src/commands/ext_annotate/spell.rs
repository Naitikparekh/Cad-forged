//! SPELL: a small built-in word list (common English plus drafting vocabulary) and a word checker.
use std::collections::HashSet;
use std::sync::OnceLock;

fn dictionary() -> &'static HashSet<&'static str> {
    static WORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    WORDS.get_or_init(|| include_str!("words.txt").lines().collect())
}

/// Remove MTEXT formatting (`\P` paragraph breaks, `\fArial;` style codes, braces).
fn strip_codes(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.next() {
                Some('P' | 'p' | 'N') => out.push(' '),
                Some('\\') => out.push('\\'),
                Some(_) => {
                    while chars.next_if(|c| *c != ';' && !c.is_whitespace()).is_some() {}
                    chars.next_if(|c| *c == ';');
                }
                None => {}
            },
            '{' | '}' => {}
            _ => out.push(ch),
        }
    }
    out
}

/// The words of a text to check: tokens made of letters only (anything containing a digit is a part number or a
/// measurement and is skipped), at least two letters long.
pub fn words(text: &str) -> Vec<String> {
    strip_codes(text)
        .split_whitespace()
        .filter(|t| !t.chars().any(|c| c.is_ascii_digit() || c == '%'))
        .flat_map(|t| {
            t.split(|c: char| !c.is_alphabetic() && c != '\'')
                .map(|w| w.trim_matches('\''))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|w| w.chars().count() >= 2)
        .collect()
}

/// Is the word in the dictionary, allowing common inflections (plural, past tense, -ing, -ly, possessive)?
pub fn known(word: &str) -> bool {
    let w = word.to_lowercase();
    let w = w.strip_suffix("'s").unwrap_or(&w);
    let dict = dictionary();
    let stems = |suffix: &str, add: &str| w.strip_suffix(suffix).map(|s| format!("{s}{add}"));
    [
        Some(w.to_string()),
        stems("s", ""),
        stems("es", ""),
        stems("ies", "y"),
        stems("ed", ""),
        stems("ed", "e"),
        stems("d", ""),
        stems("ing", ""),
        stems("ing", "e"),
        stems("ly", ""),
        stems("er", ""),
        stems("ers", ""),
        stems("est", ""),
    ]
    .into_iter()
    .flatten()
    .any(|c| dict.contains(c.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenises_and_skips_part_numbers() {
        assert_eq!(
            words("Bolt M6x20 -- THREADED rod,\\Pnext (see note) 50%"),
            vec!["Bolt", "THREADED", "rod", "next", "see", "note"]
        );
        assert_eq!(
            words("{\\fArial|b0;Hello} world's"),
            vec!["Hello", "world's"]
        );
        assert!(words("a 1 - . x2").is_empty());
    }

    #[test]
    fn known_words_and_inflections() {
        for w in [
            "door",
            "Doors",
            "THREADED",
            "dimensions",
            "drawing",
            "worlds",
            "moving",
            "quickly",
            "bolt's",
            "boxes",
            "cities",
        ] {
            assert!(known(w), "{w}");
        }
        for w in ["xyzzy", "recieve", "dimenson"] {
            assert!(!known(w), "{w}");
        }
    }
}
