//! Unicode tokenizer shared by the full-text index, `MATCH`, `bm25()`,
//! `snippet()` and `highlight()`.
//!
//! Every caller must use this one implementation: index candidates are only
//! exact when the posting lists and the per-row predicate see identical terms.
//!
//! Rules (documented in `docs/features/full-text-search.md`):
//! - A word is a maximal run of alphanumeric characters (Unicode letters and
//!   digits, plus combining marks). Everything else separates words.
//! - Words are case-folded and compatibility-normalized (NFKD): fullwidth
//!   `ＡＢＣ` becomes `abc`, ligatures split, and Latin/Greek/Cyrillic diacritics
//!   are removed (`café` == `cafe`). No stemming and no stop words.
//! - Han, kana and Hangul have no spaces between words, so a run of those
//!   characters is indexed as overlapping bigrams. Documents also index the
//!   run's final character, and a lone CJK character in a query matches as a
//!   prefix, so any substring of a CJK run is findable. Kana keep their voicing
//!   marks (NFKC), so `ガ` and `カ` stay distinct.

use unicode_normalization::char::{decompose_compatible, is_combining_mark};
use unicode_normalization::UnicodeNormalization;

/// One token with its phrase position and byte span in the source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub term: String,
    /// Phrase position. Consecutive words have consecutive positions. A CJK
    /// run's trailing unigram shares the position of the run's last bigram.
    pub position: u32,
    /// Byte offsets into the tokenized text (`start..end`).
    pub start: usize,
    pub end: usize,
    /// Query mode only: a lone CJK character matches as a prefix.
    pub prefix: bool,
}

/// Documents and queries differ only in how CJK runs are emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Document,
    Query,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Separator,
    Word,
    Cjk,
}

fn is_cjk_block(c: char) -> bool {
    matches!(
        c as u32,
        0x3005..=0x3007       // 々 〆 〇
            | 0x3040..=0x30FF // Hiragana, Katakana (incl. combining voicing marks)
            | 0x31F0..=0x31FF // Katakana phonetic extensions
            | 0x3400..=0x4DBF // CJK extension A
            | 0x4E00..=0x9FFF // CJK unified ideographs
            | 0xAC00..=0xD7AF // Hangul syllables
            | 0xF900..=0xFAFF // CJK compatibility ideographs
            | 0xFF66..=0xFF9F // Halfwidth katakana
            | 0x20000..=0x2FA1F // CJK extensions B-F, compatibility supplement
    )
}

fn classify(c: char) -> Class {
    let wordish = c.is_alphanumeric() || is_combining_mark(c);
    if !wordish {
        Class::Separator
    } else if is_cjk_block(c) {
        Class::Cjk
    } else {
        Class::Word
    }
}

/// Case-fold and strip diacritics from one word character.
fn fold_word_char(c: char, out: &mut String) {
    if c.is_ascii() {
        out.push(c.to_ascii_lowercase());
        return;
    }
    decompose_compatible(c, |d| {
        if !is_combining_mark(d) {
            out.extend(d.to_lowercase());
        }
    });
}

/// Tokenize `text`. Byte offsets refer to `text` itself.
pub fn tokenize(text: &str, mode: Mode) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut position: u32 = 0;
    let mut chars = text.char_indices().peekable();
    while let Some(&(start, c)) = chars.peek() {
        match classify(c) {
            Class::Separator => {
                chars.next();
            }
            Class::Word => {
                let mut term = String::new();
                let mut end = start;
                while let Some(&(i, d)) = chars.peek() {
                    if classify(d) != Class::Word {
                        break;
                    }
                    fold_word_char(d, &mut term);
                    end = i + d.len_utf8();
                    chars.next();
                }
                if !term.is_empty() {
                    tokens.push(Token {
                        term,
                        position,
                        start,
                        end,
                        prefix: false,
                    });
                    position += 1;
                }
            }
            Class::Cjk => {
                // (start, end, normalized character(s))
                let mut units: Vec<(usize, usize, String)> = Vec::new();
                while let Some(&(i, d)) = chars.peek() {
                    if classify(d) != Class::Cjk {
                        break;
                    }
                    chars.next();
                    let end = i + d.len_utf8();
                    let folded: String = std::iter::once(d).nfkc().collect();
                    if folded.chars().all(is_combining_mark) {
                        // Voicing mark (decomposed or halfwidth): compose
                        // with the previous character, e.g. カ + ゙ -> ガ.
                        if let Some(last) = units.last_mut() {
                            last.2 = format!("{}{}", last.2, folded).nfc().collect();
                            last.1 = end;
                        }
                        continue;
                    }
                    units.push((i, end, folded));
                }
                emit_cjk(&units, mode, &mut position, &mut tokens);
            }
        }
    }
    tokens
}

fn emit_cjk(
    units: &[(usize, usize, String)],
    mode: Mode,
    position: &mut u32,
    tokens: &mut Vec<Token>,
) {
    match units.len() {
        0 => {}
        1 => {
            let (start, end, ref term) = units[0];
            tokens.push(Token {
                term: term.clone(),
                position: *position,
                start,
                end,
                prefix: mode == Mode::Query,
            });
            *position += 1;
        }
        n => {
            for k in 0..n - 1 {
                tokens.push(Token {
                    term: format!("{}{}", units[k].2, units[k + 1].2),
                    position: *position + k as u32,
                    start: units[k].0,
                    end: units[k + 1].1,
                    prefix: false,
                });
            }
            if mode == Mode::Document {
                let (start, end, ref term) = units[n - 1];
                tokens.push(Token {
                    term: term.clone(),
                    position: *position + n as u32 - 2,
                    start,
                    end,
                    prefix: false,
                });
            }
            *position += n as u32 - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(text: &str, mode: Mode) -> Vec<String> {
        tokenize(text, mode).into_iter().map(|t| t.term).collect()
    }

    #[test]
    fn ascii_words_are_case_folded_and_split_on_punctuation() {
        assert_eq!(
            terms("Hello, World! It's v2.0_beta", Mode::Document),
            ["hello", "world", "it", "s", "v2", "0", "beta"]
        );
    }

    #[test]
    fn offsets_and_positions_point_into_the_source() {
        let text = "  Rust-lang rocks";
        let tokens = tokenize(text, Mode::Document);
        assert_eq!(tokens.len(), 3);
        assert_eq!(&text[tokens[0].start..tokens[0].end], "Rust");
        assert_eq!(&text[tokens[2].start..tokens[2].end], "rocks");
        assert_eq!(
            tokens.iter().map(|t| t.position).collect::<Vec<_>>(),
            [0, 1, 2]
        );
    }

    #[test]
    fn diacritics_fullwidth_and_unicode_case_fold() {
        assert_eq!(
            terms("Café NAÏVE ＡＢＣ１２ ÉCOLE Straße", Mode::Document),
            ["cafe", "naive", "abc12", "ecole", "straße"]
        );
        // Decomposed input folds to the same term as precomposed input.
        assert_eq!(terms("cafe\u{301}", Mode::Document), ["cafe"]);
    }

    #[test]
    fn cjk_runs_become_bigrams_with_trailing_unigram() {
        let tokens = tokenize("東京タワー", Mode::Document);
        let got: Vec<_> = tokens
            .iter()
            .map(|t| (t.term.as_str(), t.position))
            .collect();
        assert_eq!(
            got,
            [
                ("東京", 0),
                ("京タ", 1),
                ("タワ", 2),
                ("ワー", 3),
                ("ー", 3)
            ]
        );
        assert_eq!(
            terms("東京タワー", Mode::Query),
            ["東京", "京タ", "タワ", "ワー"]
        );
        let single = tokenize("東", Mode::Query);
        assert!(single[0].prefix);
    }

    #[test]
    fn mixed_scripts_and_kana_normalization() {
        assert_eq!(
            terms("東京2020 olympics", Mode::Document),
            ["東京", "京", "2020", "olympics"]
        );
        // Halfwidth katakana with a separate voicing mark == fullwidth ガ.
        assert_eq!(terms("ｶﾞ", Mode::Document), ["ガ"]);
        assert_eq!(terms("カ\u{3099}", Mode::Document), ["ガ"]);
        assert_ne!(terms("ガ", Mode::Document), terms("カ", Mode::Document));
    }

    #[test]
    fn never_panics_on_arbitrary_text() {
        for text in [
            "",
            "   ",
            "\u{301}\u{301}",
            "日本語のテキストです。句読点、「括弧」も。",
            "emoji 🚀🔥 mixed ｶﾀｶﾅ and 한국어 텍스트",
            "\u{0}\u{7f}\u{feff}",
        ] {
            let tokens = tokenize(text, Mode::Document);
            for t in &tokens {
                assert!(t.start < t.end && t.end <= text.len());
                assert!(text.is_char_boundary(t.start) && text.is_char_boundary(t.end));
            }
            tokenize(text, Mode::Query);
        }
    }
}
