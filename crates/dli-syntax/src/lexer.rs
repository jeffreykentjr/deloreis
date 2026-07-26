//! Lexer.
//!
//! Hand-rolled and line-oriented (DL-0008). A `.dli` file is a sequence of
//! physical lines; a leading `#` (outside a string) is a comment to end of
//! line. Within a line the lexer emits words, integers, string literals, and
//! the punctuation the record grammar needs — `( ) , = { }` — with a byte
//! [`Span`] on every token. Operators the expert tier writes inside clause
//! bodies (`<`, `->`, `:-`, `>=`) are ordinary word characters: they fall out
//! as their own space-delimited words and are read from raw text by the lanes
//! that own them, so the core lexer stays tiny.

use dli_ast::span::{FileId, Span};

/// Byte offsets are `usize`; spans hold `u32`. Sources are small, so a saturating
/// conversion is exact in practice and never panics.
pub(crate) fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// The kind of a lexical token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// A bareword: a keyword, identifier, or operator run.
    Word,
    /// An integer literal (a maximal run of ASCII digits).
    Int,
    /// A double-quoted string literal; the token text is the unquoted content.
    Str,
    /// `(`.
    LParen,
    /// `)`.
    RParen,
    /// `,`.
    Comma,
    /// `=`.
    Equals,
    /// `{`.
    LBrace,
    /// `}`.
    RBrace,
    /// The end of a physical line.
    Newline,
}

/// A lexical token with its source span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// The token's kind.
    pub kind: TokenKind,
    /// The token text. For [`TokenKind::Str`] this is the unquoted content.
    pub text: String,
    /// The token's source span (byte offsets; for a `Newline`, an empty span at
    /// the line end).
    pub span: Span,
}

/// One physical line, prepared for the parser: its indent, comment-stripped
/// content, and tokenization. A line whose `raw` is empty is blank.
#[derive(Debug, Clone)]
pub(crate) struct Line {
    /// Leading-whitespace width (columns).
    pub indent: usize,
    /// The content after the indent, comment-stripped and right-trimmed.
    pub raw: String,
    /// The tokenization of `raw`.
    pub tokens: Vec<Token>,
    /// The span over `raw`.
    pub span: Span,
}

impl Line {
    /// Whether this line is blank (only whitespace and/or a comment).
    pub(crate) fn is_blank(&self) -> bool {
        self.raw.is_empty()
    }
}

/// Tokenize source into a flat token stream, one `Newline` per physical line.
///
/// Comments are dropped. This is the raw lexical view; the parser consumes the
/// higher-level line grouping built from the same scan.
///
/// ```
/// use dli_syntax::lexer::{lex, TokenKind};
/// use dli_ast::span::FileId;
///
/// let tokens = lex("place saltmarket \"Saltmarket\"\n", FileId(0));
/// let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
/// assert_eq!(
///     kinds,
///     vec![TokenKind::Word, TokenKind::Word, TokenKind::Str, TokenKind::Newline],
/// );
/// assert_eq!(tokens[2].text, "Saltmarket");
/// ```
#[must_use]
pub fn lex(source: &str, file: FileId) -> Vec<Token> {
    let mut out = Vec::new();
    for line in lines(source, file) {
        let end = line.span.end;
        out.extend(line.tokens);
        out.push(Token {
            kind: TokenKind::Newline,
            text: String::new(),
            span: Span {
                file,
                start: end,
                end,
            },
        });
    }
    out
}

/// The line-grouped view of source used by the parser.
pub(crate) fn lines(source: &str, file: FileId) -> Vec<Line> {
    let mut result = Vec::new();
    let mut offset: u32 = 0;
    for physical in source.split_inclusive('\n') {
        let without_nl = physical.strip_suffix('\n').unwrap_or(physical);
        let indent = without_nl
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .count();
        let after_indent = &without_nl[indent..];
        let content = strip_comment(after_indent).trim_end();
        let content_start = offset + to_u32(indent);
        let span = Span {
            file,
            start: content_start,
            end: content_start + to_u32(content.len()),
        };
        let tokens = tokenize(content, content_start, file);
        result.push(Line {
            indent,
            raw: content.to_owned(),
            tokens,
            span,
        });
        offset += to_u32(physical.len());
    }
    result
}

/// Return the prefix of `line` before an unquoted `#` comment marker.
fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            _ if escaped => escaped = false,
            b'\\' if in_string => escaped = true,
            b'"' => in_string = !in_string,
            b'#' if !in_string => return &line[..i],
            _ => {}
        }
        i += 1;
    }
    line
}

/// Tokenize a single comment-stripped line body starting at `base` bytes.
fn tokenize(content: &str, base: u32, file: FileId) -> Vec<Token> {
    let mut tokens = Vec::new();
    let bytes = content.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b' ' || c == b'\t' {
            i += 1;
            continue;
        }
        let start = i;
        let kind = match c {
            b'"' => {
                i += 1;
                let text_start = i;
                while i < bytes.len() && bytes[i] != b'"' {
                    // Skip an escaped character so `\"` does not close the string.
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        i += 1;
                    }
                    i += 1;
                }
                let text = unescape(&content[text_start..i]);
                if i < bytes.len() {
                    i += 1; // consume the closing quote
                }
                tokens.push(Token {
                    kind: TokenKind::Str,
                    text,
                    span: Span {
                        file,
                        start: base + to_u32(start),
                        end: base + to_u32(i),
                    },
                });
                continue;
            }
            b'(' => TokenKind::LParen,
            b')' => TokenKind::RParen,
            b',' => TokenKind::Comma,
            b'=' => TokenKind::Equals,
            b'{' => TokenKind::LBrace,
            b'}' => TokenKind::RBrace,
            _ => {
                while i < bytes.len() && !is_delimiter(bytes[i]) {
                    i += 1;
                }
                let text = &content[start..i];
                let kind = if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) {
                    TokenKind::Int
                } else {
                    TokenKind::Word
                };
                tokens.push(Token {
                    kind,
                    text: text.to_owned(),
                    span: Span {
                        file,
                        start: base + to_u32(start),
                        end: base + to_u32(i),
                    },
                });
                continue;
            }
        };
        i += 1;
        tokens.push(Token {
            kind,
            text: content[start..i].to_owned(),
            span: Span {
                file,
                start: base + to_u32(start),
                end: base + to_u32(i),
            },
        });
    }
    tokens
}

/// Whether `b` ends a bareword.
fn is_delimiter(b: u8) -> bool {
    matches!(
        b,
        b' ' | b'\t' | b'(' | b')' | b',' | b'=' | b'{' | b'}' | b'"'
    )
}

/// Resolve `\"` and `\\` escapes inside a string literal body.
fn unescape(raw: &str) -> String {
    if !raw.contains('\\') {
        return raw.to_owned();
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(next) => out.push(next),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}
