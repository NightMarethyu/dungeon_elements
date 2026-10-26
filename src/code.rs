//! Syntax colors for code blocks in the reading view.
//!
//! [syntect] finds the parts of the code (keywords, strings, comments...) with the same
//! grammars many editors use, and the app's theme gives each kind of part its color, so
//! code blocks follow the theme, custom ones too.

use std::ops::Range;
use std::str::FromStr;
use std::sync::OnceLock;

use iced::Color;
use syntect::easy::HighlightLines;
use syntect::highlighting::{self, ScopeSelectors, StyleModifier, ThemeItem, ThemeSettings};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::theme::Colors;

/// A kind of part of the code, which the theme gives a color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Plain,
    Comment,
    String,
    /// Numbers and constants like `true` and `null`.
    Number,
    Keyword,
    Function,
    /// Types, classes, and HTML and XML tags.
    Type,
}

impl Token {
    const ALL: [Token; 7] = [
        Token::Plain,
        Token::Comment,
        Token::String,
        Token::Number,
        Token::Keyword,
        Token::Function,
        Token::Type,
    ];

    /// The color the theme gives this kind of part, or `None` for the text's own color.
    pub fn color(self, c: Colors) -> Option<Color> {
        match self {
            Token::Plain => None,
            Token::Comment => Some(c.code_comment),
            Token::String => Some(c.code_string),
            Token::Number => Some(c.code_number),
            Token::Keyword => Some(c.code_keyword),
            Token::Function => Some(c.code_function),
            Token::Type => Some(c.code_type),
        }
    }

    /// The grammar scopes that are this kind of part.
    fn scopes(self) -> &'static str {
        match self {
            // Operators and punctuation are keywords to some grammars, but plain here.
            Token::Plain => "keyword.operator, punctuation, meta.brace",
            Token::Comment => "comment, punctuation.definition.comment",
            Token::String => {
                "string, constant.character, punctuation.definition.string, markup.inline.raw"
            }
            Token::Number => {
                "constant.numeric, constant.language, constant.other, support.constant, \
                 variable.other.constant"
            }
            Token::Keyword => "keyword, storage, variable.language, markup.heading",
            Token::Function => {
                "entity.name.function, support.function, variable.function, \
                 meta.function-call.generic"
            }
            Token::Type => {
                "entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, \
                 entity.name.tag, entity.other.inherited-class, support.type, support.class, \
                 storage.type.primitive, entity.other.attribute-name"
            }
        }
    }
}

/// One line of code as byte ranges in it, without the line break, and what each one is.
pub type Line = Vec<(Range<usize>, Token)>;

/// Finds the parts of `code`, written in `language` (a name or file extension like
/// `rust`, `py`, or `yaml`). Returns `None` for a language it doesn't know.
pub fn highlight(language: &str, code: &str) -> Option<Vec<Line>> {
    let syntaxes = syntaxes();
    // Fences can say more after the language, like "rust,ignore".
    let language = language.split([',', ' ']).next()?.trim();
    let syntax = syntaxes
        .find_syntax_by_token(language)
        .or_else(|| syntaxes.find_syntax_by_extension(&language.to_ascii_lowercase()))?;
    let mut highlighter = HighlightLines::new(syntax, theme());

    let mut lines = Vec::new();
    for line in LinesWithEndings::from(code) {
        let regions = highlighter.highlight_line(line, syntaxes).ok()?;
        let length = line.trim_end_matches(['\n', '\r']).len();
        let mut parts: Line = Vec::new();
        let mut start = 0;
        for (style, text) in regions {
            let end = (start + text.len()).min(length);
            let token = Token::ALL
                .get(usize::from(style.foreground.r))
                .copied()
                .unwrap_or(Token::Plain);
            if start < end {
                match parts.last_mut() {
                    Some((range, last)) if *last == token => range.end = end,
                    _ => parts.push((start..end, token)),
                }
            }
            start += text.len();
        }
        lines.push(parts);
    }
    Some(lines)
}

/// The grammars that come with syntect: most common languages, YAML and Markdown too.
fn syntaxes() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// A syntect theme whose colors aren't colors: each one's red channel is the index of a
/// [`Token`], so the code's parts can be colored by whichever theme is in use later.
fn theme() -> &'static highlighting::Theme {
    static THEME: OnceLock<highlighting::Theme> = OnceLock::new();
    THEME.get_or_init(|| {
        let marker = |token: Token| highlighting::Color {
            r: Token::ALL.iter().position(|t| *t == token).unwrap_or(0) as u8,
            g: 0,
            b: 0,
            a: 255,
        };
        highlighting::Theme {
            settings: ThemeSettings {
                foreground: Some(marker(Token::Plain)),
                ..ThemeSettings::default()
            },
            scopes: Token::ALL
                .iter()
                .map(|&token| ThemeItem {
                    scope: ScopeSelectors::from_str(token.scopes()).expect("valid scopes"),
                    style: StyleModifier {
                        foreground: Some(marker(token)),
                        background: None,
                        font_style: None,
                    },
                })
                .collect(),
            ..highlighting::Theme::default()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(language: &str, code: &str) -> Vec<(String, Token)> {
        let lines = highlight(language, code).unwrap();
        code.lines()
            .zip(lines)
            .flat_map(|(line, parts)| {
                parts
                    .into_iter()
                    .map(move |(range, token)| (line[range].to_string(), token))
            })
            .filter(|(_, token)| *token != Token::Plain)
            .collect()
    }

    #[test]
    fn colors_code() {
        let rust = parts("rust", "// hi\nfn roll() -> u8 { 20 }\nlet s = \"d20\";\n");
        assert!(rust.contains(&("// hi".into(), Token::Comment)));
        assert!(rust.contains(&("fn".into(), Token::Keyword)));
        assert!(rust.contains(&("roll".into(), Token::Function)));
        assert!(rust.contains(&("20".into(), Token::Number)));
        assert!(
            rust.iter()
                .any(|(text, token)| text.contains("d20") && *token == Token::String)
        );

        let yaml = parts("YAML", "hp: 12 # full\n");
        assert!(yaml.contains(&("12".into(), Token::Number)));
        assert!(yaml.iter().any(|(_, token)| *token == Token::Comment));
    }

    #[test]
    fn knows_languages_by_extension_and_skips_unknown_ones() {
        assert!(highlight("py", "x = 1\n").is_some());
        assert!(highlight("rust,ignore", "x\n").is_some());
        assert!(highlight("elvish-runes", "x\n").is_none());
    }
}
