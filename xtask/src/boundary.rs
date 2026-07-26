//! The boundary guard: engine crates must not hardcode a world's proper nouns in
//! real (non-comment, non-test) source.
//!
//! dli ships as a general-purpose lore engine; "To Pleach the Hedgeway" (`TPtH`) is
//! one world authored in it. The separation plan
//! (`docs/design-notes/tpth-separation-plan.md`, Phase 1) makes that boundary a
//! gate check: no engine crate (`crates/dli-*`, `crates/deloreis`) may name a `TPtH`
//! proper noun in real code. Doc-comment examples, doctests, and `#[cfg(test)]`
//! code are acceptable illustration and are skipped; a token in a real function
//! body, a real string literal, or a `const` is a genuine content leak and fails
//! the build.
//!
//! Today's known real-code leaks are the `ALLOWLIST` — tracked debt, each with a
//! removal note. A one-off may instead carry a `// tpth-exempt: <reason>` marker
//! on the offending line. Everything else fails.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use walkdir::WalkDir;

/// `TPtH` world proper-noun tokens forbidden in engine real code. Matched
/// whole-word (ASCII boundary) and case-insensitively; keep them lowercase.
/// `D-nnnn` decision ids are deliberately absent — they legitimately appear in
/// mechanism doc-comments.
///
/// This is the canonical superset: it merges the original 19 boundary tokens,
/// the `dli-world` craft lint's `FORBIDDEN_WORLD_TOKENS`, and live canon names
/// present in fixtures. Engine real code is TPtH-free within the proven scope
/// (canonical tokens + spaced forms).
pub const WORLD_TOKENS: &[&str] = &[
    // --- original 19 ---
    "enfys",
    "aalto",
    "the-debt-square",
    "weather-debt",
    "saltmarket",
    "the-shallows",
    "cymric",
    "the-turning",
    "lowlands",
    "reckoner",
    "concordance",
    "spoken-proof",
    "pleach",
    "hedgeway",
    "the-multitude",
    "gwern",
    "tide-bell",
    "mara",
    "breakwater",
    // --- from FORBIDDEN_WORLD_TOKENS (dli-world craft lint) ---
    "turning",
    "proof-speech",
    "wyrd",
    "multitude",
    // --- live canon names (present in fixtures) ---
    "tegwen",
    "kes",
    "branwen",
];

/// A permitted current leak: real-engine-code uses of specific world tokens in one
/// file, tracked as debt with a removal note. A flagged line in `file` is exempt
/// only if EVERY world token it names is in `tokens`; a leak of any *other* token
/// in the same file still fails the guard.
#[allow(dead_code)] // the tracked-debt allowlist mechanism
struct Allow {
    /// Repo-relative path (forward slashes).
    file: &'static str,
    /// The world tokens tolerated in this file (lowercase).
    tokens: &'static [&'static str],
    /// One line on how the leak is removed.
    note: &'static str,
}

/// The tracked-debt allowlist — currently EMPTY. Engine real code is TPtH-free
/// within the proven scope (26 canonical tokens + spaced forms): no `## Section`
/// cites a world token in a non-comment, non-test line. An entry here (or a
/// `// tpth-exempt:` marker) is how a genuine, temporary leak would be tracked;
/// none exist today.
const ALLOWLIST: &[Allow] = &[];

/// One real-code leak: a forbidden token in a real (non-comment, non-test) line.
struct Leak {
    file: String,
    line_no: usize,
    tokens: Vec<&'static str>,
    text: String,
}

/// Scan the engine crates and fail if any forbidden world token appears in real
/// code outside the allowlist or a `// tpth-exempt:` marker.
pub fn run_boundary(root: &Path) -> Result<()> {
    let files = engine_source_files(root);
    let scanned = files.len();
    let mut leaks: Vec<Leak> = Vec::new();

    for path in files {
        let text = std::fs::read_to_string(&path)
            .map_err(|err| anyhow::anyhow!("reading {}: {err}", path.display()))?;
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        scan_file(&rel, &text, &mut leaks);
    }

    if leaks.is_empty() {
        println!(
            "boundary: OK — no TPtH world tokens in engine real code \
             ({scanned} files scanned, {} allowlisted debt site(s))",
            ALLOWLIST.len()
        );
        for entry in ALLOWLIST {
            println!(
                "boundary:   debt {} [{}] — {}",
                entry.file,
                entry.tokens.join(", "),
                entry.note
            );
        }
        return Ok(());
    }

    for leak in &leaks {
        let names = leak
            .tokens
            .iter()
            .map(|token| format!("`{token}`"))
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!(
            "boundary: LEAK {}:{}: world token {names} in real engine code\n    {}",
            leak.file,
            leak.line_no,
            leak.text.trim()
        );
    }
    eprintln!(
        "boundary: extract the content to worlds/tpth (or a generic name), or record it as \
         tracked debt in the xtask boundary allowlist / a `// tpth-exempt:` marker."
    );
    bail!(
        "boundary: {} world-token leak(s) in engine real code",
        leaks.len()
    );
}

/// The `.rs` files under `crates/dli-*/src` and `crates/deloreis/src`. The world
/// crate (`crates/tpth`), `worlds/`, and any `tests/` directory are not engine
/// real code and are excluded.
fn engine_source_files(root: &Path) -> Vec<PathBuf> {
    let crates_dir = root.join("crates");
    let mut files: Vec<PathBuf> = Vec::new();

    for entry in WalkDir::new(&crates_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let Ok(rel) = path.strip_prefix(&crates_dir) else {
            continue;
        };
        let parts: Vec<String> = rel
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect();
        // parts == [<crate>, src, ...]; engine == dli-* or deloreis, under src/.
        let is_engine_crate = parts
            .first()
            .is_some_and(|name| name.starts_with("dli-") || name == "deloreis");
        let under_src = parts.get(1).map(String::as_str) == Some("src");
        let in_tests = parts.iter().any(|part| part == "tests");
        if is_engine_crate && under_src && !in_tests {
            files.push(path.to_path_buf());
        }
    }
    files.sort();
    files
}

/// Scan one engine source file, appending any real-code leaks.
fn scan_file(rel: &str, text: &str, leaks: &mut Vec<Leak>) {
    let allowed = allowed_tokens(rel);
    let mut depth: i64 = 0;
    // `Some(d)` while inside a `#[cfg(test)]` block that closes when depth <= d.
    let mut test_close_depth: Option<i64> = None;
    let mut pending_cfg = false;
    let mut lex_state = LexState::default();

    for (index, line) in text.lines().enumerate() {
        let lexed = lex_line(line, &mut lex_state);
        let has_cfg_test = lexed
            .cfg_test_positions
            .iter()
            .any(|position| cfg_is_item_attribute(&lexed.visible[..*position]));
        if has_cfg_test {
            pending_cfg = true;
        }

        let mut outside_tests = vec![b' '; lexed.visible.len()];
        let visible = lexed.visible.as_bytes();
        let mut cursor = 0;
        let mut in_test = pending_cfg || test_close_depth.is_some();
        for event in &lexed.events {
            if !in_test {
                outside_tests[cursor..event.offset].copy_from_slice(&visible[cursor..event.offset]);
            }
            match event.kind {
                CodeEventKind::OpenBrace => {
                    if pending_cfg && test_close_depth.is_none() {
                        test_close_depth = Some(depth);
                        pending_cfg = false;
                        in_test = true;
                    }
                    depth += 1;
                }
                CodeEventKind::CloseBrace => {
                    depth -= 1;
                    if test_close_depth.is_some_and(|close| depth <= close) {
                        test_close_depth = None;
                        in_test = pending_cfg;
                    }
                }
                CodeEventKind::Semicolon if pending_cfg && test_close_depth.is_none() => {
                    pending_cfg = false;
                    in_test = false;
                }
                CodeEventKind::Semicolon => {}
            }
            if !in_test {
                outside_tests[event.offset] = visible[event.offset];
            }
            cursor = event.offset + 1;
        }
        if !in_test {
            outside_tests[cursor..].copy_from_slice(&visible[cursor..]);
        }

        if !lexed.has_exemption {
            let visible = String::from_utf8(outside_tests).unwrap_or_default();
            let hits = line_tokens(&visible);
            if !hits.is_empty() {
                let disallowed: Vec<&'static str> = hits
                    .into_iter()
                    .filter(|token| !allowed.contains(token))
                    .collect();
                if !disallowed.is_empty() {
                    leaks.push(Leak {
                        file: rel.to_owned(),
                        line_no: index + 1,
                        tokens: disallowed,
                        text: line.to_owned(),
                    });
                }
            }
        }
    }
}

fn cfg_is_item_attribute(prefix: &str) -> bool {
    let mut rest = prefix.trim();
    while rest.starts_with("#[") {
        let Some(end) = rest.find(']') else {
            return false;
        };
        rest = rest[end + 1..].trim_start();
    }
    rest.is_empty()
}

#[derive(Clone, Copy, Default)]
enum LexMode {
    #[default]
    Code,
    BlockComment(usize),
    String,
    RawString(usize),
}

#[derive(Default)]
struct LexState {
    mode: LexMode,
}

struct LexedLine {
    visible: String,
    events: Vec<CodeEvent>,
    cfg_test_positions: Vec<usize>,
    has_exemption: bool,
}

struct CodeEvent {
    offset: usize,
    kind: CodeEventKind,
}

#[derive(Clone, Copy)]
enum CodeEventKind {
    OpenBrace,
    CloseBrace,
    Semicolon,
}

/// Classify one Rust source line while carrying multiline lexical state.
///
/// Comments are blanked from `visible`; string literal bytes remain visible so
/// world tokens in executable strings are still checked. Attributes and braces
/// are recognized only in code. Rust block comments nest, and raw strings may
/// span lines without exposing comment delimiters, attributes, or braces.
fn lex_line(line: &str, state: &mut LexState) -> LexedLine {
    const ATTRIBUTE: &[u8] = b"#[cfg(test)]";
    let bytes = line.as_bytes();
    let mut visible = vec![b' '; bytes.len()];
    let mut events = Vec::new();
    let mut cfg_test_positions = Vec::new();
    let mut has_exemption = false;
    let mut i = 0;

    while i < bytes.len() {
        match state.mode {
            LexMode::BlockComment(mut depth) => {
                if bytes[i..].starts_with(b"/*") {
                    depth += 1;
                    state.mode = LexMode::BlockComment(depth);
                    i += 2;
                } else if bytes[i..].starts_with(b"*/") {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        state.mode = LexMode::Code;
                    } else {
                        state.mode = LexMode::BlockComment(depth);
                    }
                } else {
                    i += 1;
                }
            }
            LexMode::String => {
                let (end, closed) = skip_string(bytes, i);
                visible[i..end].copy_from_slice(&bytes[i..end]);
                if closed {
                    state.mode = LexMode::Code;
                }
                i = end;
            }
            LexMode::RawString(hashes) => {
                let raw_end = raw_string_end(bytes, i, hashes);
                let end = raw_end.unwrap_or(bytes.len());
                visible[i..end].copy_from_slice(&bytes[i..end]);
                if raw_end.is_some() {
                    state.mode = LexMode::Code;
                }
                i = end;
            }
            LexMode::Code => {
                if bytes[i..].starts_with(b"//") {
                    let comment = std::str::from_utf8(&bytes[i + 2..])
                        .unwrap_or_default()
                        .trim();
                    has_exemption = comment
                        .strip_prefix("tpth-exempt:")
                        .is_some_and(|reason| !reason.trim().is_empty());
                    break;
                }
                if bytes[i..].starts_with(b"/*") {
                    state.mode = LexMode::BlockComment(1);
                    i += 2;
                    continue;
                }
                if let Some((body_start, hashes)) = raw_string_start(bytes, i) {
                    let raw_end = raw_string_end(bytes, body_start, hashes);
                    let end = raw_end.unwrap_or(bytes.len());
                    visible[i..end].copy_from_slice(&bytes[i..end]);
                    if raw_end.is_none() {
                        state.mode = LexMode::RawString(hashes);
                    }
                    i = end;
                    continue;
                }
                match bytes[i] {
                    b'"' => {
                        let (end, closed) = skip_string(bytes, i + 1);
                        visible[i..end].copy_from_slice(&bytes[i..end]);
                        if !closed {
                            state.mode = LexMode::String;
                        }
                        i = end;
                    }
                    b'\'' => {
                        let end = skip_char_or_lifetime(bytes, i).min(bytes.len());
                        visible[i..end].copy_from_slice(&bytes[i..end]);
                        i = end;
                    }
                    b'{' => {
                        visible[i] = bytes[i];
                        events.push(CodeEvent {
                            offset: i,
                            kind: CodeEventKind::OpenBrace,
                        });
                        i += 1;
                    }
                    b'}' => {
                        visible[i] = bytes[i];
                        events.push(CodeEvent {
                            offset: i,
                            kind: CodeEventKind::CloseBrace,
                        });
                        i += 1;
                    }
                    b';' => {
                        visible[i] = bytes[i];
                        events.push(CodeEvent {
                            offset: i,
                            kind: CodeEventKind::Semicolon,
                        });
                        i += 1;
                    }
                    _ => {
                        if bytes[i..].starts_with(ATTRIBUTE) {
                            cfg_test_positions.push(i);
                        }
                        visible[i] = bytes[i];
                        i += 1;
                    }
                }
            }
        }
    }

    LexedLine {
        visible: String::from_utf8(visible).unwrap_or_else(|_| line.to_owned()),
        events,
        cfg_test_positions,
        has_exemption,
    }
}

/// The union of tolerated tokens for a file across all allowlist entries.
fn allowed_tokens(rel: &str) -> Vec<&'static str> {
    ALLOWLIST
        .iter()
        .filter(|entry| entry.file == rel)
        .flat_map(|entry| entry.tokens.iter().copied())
        .collect()
}

/// The forbidden world tokens named on a line (whole-word, case-insensitive),
/// after blanking any `worlds/tpth/...` data path (a path is not content).
/// For hyphenated tokens starting with `the-`, also checks the spaced equivalent
/// (F5S-1): e.g. `the-turning` also scans for `the turning`. Only `the-` prefixed
/// tokens get spaced-form expansion since other hyphenated tokens (`spoken-proof`,
/// `proof-speech`) could appear as generic English.
fn line_tokens(line: &str) -> Vec<&'static str> {
    let lower = blank_world_paths(&line.to_lowercase());
    WORLD_TOKENS
        .iter()
        .copied()
        .filter(|token| {
            if contains_word(&lower, token) {
                return true;
            }
            // F5S-1: for `the-X` tokens, also check the spaced form
            // ("the turning", "the multitude", etc.).
            if token.starts_with("the-") {
                let spaced = token.replace('-', " ");
                contains_word(&lower, &spaced)
            } else {
                false
            }
        })
        .collect()
}

/// Replace `worlds/tpth/<path>` runs with spaces so a data-path string literal is
/// never read as content. Input is already lowercased; only ASCII path bytes are
/// blanked, so surrounding multibyte text is preserved.
fn blank_world_paths(lower: &str) -> String {
    const MARKER: &[u8] = b"worlds/tpth";
    let bytes = lower.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(MARKER) {
            let mut end = i + MARKER.len();
            while end < bytes.len() && is_path_byte(bytes[end]) {
                end += 1;
            }
            out.resize(out.len() + (end - i), b' ');
            i = end;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| lower.to_owned())
}

/// A byte that may continue a `worlds/tpth/...` path (all ASCII).
fn is_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-')
}

/// Whole-word (ASCII-boundary), case-insensitive containment. `haystack` is
/// lowercased and `needle` is lowercase; a match must be flanked by non-ASCII
/// -alphanumeric bytes (or the ends). Mirrors the craft lint's `contains_word`.
pub fn contains_word(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let mut from = 0;
    while let Some(rel) = haystack[from..].find(needle) {
        let i = from + rel;
        let end = i + needle.len();
        let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
        let after_ok = end >= bytes.len() || !bytes[end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        from = i + 1;
    }
    false
}

/// Advance past a double-quoted string body, returning the index after the
/// closing quote (or end of line) and whether a closing quote was found.
fn skip_string(bytes: &[u8], mut i: usize) -> (usize, bool) {
    let len = bytes.len();
    while i < len {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return (i + 1, true),
            _ => i += 1,
        }
    }
    (len, false)
}

/// At `i`, recognize an `r"..."`, `r#"..."#`, or byte-raw `br#"..."#`
/// opener. Returns the raw body start and delimiter hash count.
fn raw_string_start(bytes: &[u8], i: usize) -> Option<(usize, usize)> {
    let mut cursor = match bytes.get(i..) {
        Some(rest) if rest.starts_with(b"r") => i + 1,
        Some(rest) if rest.starts_with(b"br") => i + 2,
        _ => return None,
    };
    let mut hashes = 0;
    while bytes.get(cursor) == Some(&b'#') {
        hashes += 1;
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'"') {
        Some((cursor + 1, hashes))
    } else {
        None
    }
}

/// Find the index after a raw string's closing quote and exact hash delimiter.
fn raw_string_end(bytes: &[u8], mut i: usize, hashes: usize) -> Option<usize> {
    while i < bytes.len() {
        if bytes[i] == b'"'
            && bytes
                .get(i + 1..i + 1 + hashes)
                .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
        {
            return Some(i + 1 + hashes);
        }
        i += 1;
    }
    None
}

/// At a `'`, advance past a char/byte literal, or one byte if it is a lifetime or
/// label. Returns the next index to scan.
fn skip_char_or_lifetime(bytes: &[u8], i: usize) -> usize {
    let len = bytes.len();
    if i + 1 < len && bytes[i + 1] == b'\\' {
        // Escaped char literal, e.g. '\n' or '\u{7f}': run to the closing quote.
        let mut j = i + 2;
        while j < len {
            match bytes[j] {
                b'\\' => j += 2,
                b'\'' => return j + 1,
                _ => j += 1,
            }
        }
        return len;
    }
    if i + 2 < len && bytes[i + 2] == b'\'' {
        // Simple char literal, e.g. 'x' or '{'.
        return i + 3;
    }
    // A lifetime (`'a`) or label: treat the quote as ordinary punctuation.
    i + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_comment_cannot_hide_the_following_world_token() {
        let mut leaks = Vec::new();
        scan_file(
            "crates/dli-world/src/example.rs",
            "/* #[cfg(test)] */\nconst LEAK: &str = \"mara\";\n",
            &mut leaks,
        );
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].line_no, 2);
    }

    #[test]
    fn block_comment_cannot_hide_code_after_its_closer() {
        let mut leaks = Vec::new();
        scan_file(
            "crates/dli-world/src/example.rs",
            "/* comment\n*/ const LEAK: &str = \"mara\";\n",
            &mut leaks,
        );
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].line_no, 2);
    }

    #[test]
    fn nested_comments_neither_expose_inner_tokens_nor_hide_following_code() {
        let mut leaks = Vec::new();
        scan_file(
            "crates/dli-world/src/example.rs",
            "/* outer\n\
               /* inner */\n\
               #[cfg(test)] mod fake { const HIDDEN: &str = \"mara\"; }\n\
             */ const VISIBLE: &str = \"gwern\";\n",
            &mut leaks,
        );
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].line_no, 4);
        assert_eq!(leaks[0].tokens, vec!["gwern"]);
    }

    #[test]
    fn raw_string_syntax_is_content_but_does_not_change_structure() {
        let mut leaks = Vec::new();
        scan_file(
            "crates/dli-world/src/example.rs",
            "const RAW: &str = r###\"// tpth-exempt /* */ #[cfg(test)] { mara }\"###;\n\
             #[cfg(test)]\n\
             mod tests {\n\
                 const EXEMPT: &str = \"tegwen\";\n\
             }\n\
             const AFTER: &str = \"gwern\";\n",
            &mut leaks,
        );
        assert_eq!(leaks.len(), 2);
        assert_eq!(leaks[0].line_no, 1);
        assert_eq!(leaks[0].tokens, vec!["mara"]);
        assert_eq!(leaks[1].line_no, 6);
        assert_eq!(leaks[1].tokens, vec!["gwern"]);
    }

    #[test]
    fn same_line_test_item_with_balanced_braces_is_exempt() {
        let mut leaks = Vec::new();
        scan_file(
            "crates/dli-world/src/example.rs",
            "#[cfg(test)] fn fixture() { let name = \"mara\"; }\n\
             const AFTER: &str = \"gwern\";\n",
            &mut leaks,
        );
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].line_no, 2);
        assert_eq!(leaks[0].tokens, vec!["gwern"]);
    }

    #[test]
    fn scanning_resumes_after_a_test_item_closes_on_the_same_line() {
        let mut leaks = Vec::new();
        scan_file(
            "crates/dli-world/src/example.rs",
            "#[cfg(test)]\n\
             mod tests {} const LEAK: &str = \"mara\";\n",
            &mut leaks,
        );
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].line_no, 2);
        assert_eq!(leaks[0].tokens, vec!["mara"]);
    }

    #[test]
    fn cfg_tokens_inside_macros_are_not_item_attributes() {
        let mut leaks = Vec::new();
        scan_file(
            "crates/dli-world/src/example.rs",
            "const LEAK: &str = stringify!(#[cfg(test)] mara);\n",
            &mut leaks,
        );
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].tokens, vec!["mara"]);
    }

    #[test]
    fn scanning_resumes_after_a_cfg_nonblock_item_semicolon() {
        let mut leaks = Vec::new();
        scan_file(
            "crates/dli-world/src/example.rs",
            "#[cfg(test)] use example::mara; const LEAK: &str = \"gwern\";\n",
            &mut leaks,
        );
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].tokens, vec!["gwern"]);
    }

    #[test]
    fn exemption_requires_the_exact_marker_and_a_reason() {
        for comment in ["notpth-exempt-not-a-marker", "tpth-exempt:", "tpth-exempt"] {
            let mut leaks = Vec::new();
            let source = format!("const LEAK: &str = \"mara\"; // {comment}\n");
            scan_file("crates/dli-world/src/example.rs", &source, &mut leaks);
            assert_eq!(leaks.len(), 1, "{comment}");
        }

        let mut leaks = Vec::new();
        scan_file(
            "crates/dli-world/src/example.rs",
            "const DEBT: &str = \"mara\"; // tpth-exempt: tracked migration seam\n",
            &mut leaks,
        );
        assert!(leaks.is_empty());
    }

    #[test]
    fn ordinary_multiline_string_does_not_expose_syntax() {
        let mut leaks = Vec::new();
        scan_file(
            "crates/dli-world/src/example.rs",
            "const TEXT: &str = \"\n\
             #[cfg(test)] { mara }\n\
             \";\n\
             const AFTER: &str = \"gwern\";\n",
            &mut leaks,
        );
        assert_eq!(leaks.len(), 2);
        assert_eq!(leaks[0].line_no, 2);
        assert_eq!(leaks[0].tokens, vec!["mara"]);
        assert_eq!(leaks[1].line_no, 4);
        assert_eq!(leaks[1].tokens, vec!["gwern"]);
    }

    #[test]
    fn multiline_byte_raw_string_syntax_is_content_and_scanning_resumes() {
        let mut leaks = Vec::new();
        scan_file(
            "crates/dli-world/src/example.rs",
            "const RAW: &[u8] = br##\"/* {\n\
             #[cfg(test)] tegwen // }\n\
             */ }\"##; const SAME_LINE: &str = \"mara\";\n\
             const AFTER: &str = \"gwern\";\n",
            &mut leaks,
        );
        assert_eq!(leaks.len(), 3);
        assert_eq!(leaks[0].line_no, 2);
        assert_eq!(leaks[0].tokens, vec!["tegwen"]);
        assert_eq!(leaks[1].line_no, 3);
        assert_eq!(leaks[1].tokens, vec!["mara"]);
        assert_eq!(leaks[2].line_no, 4);
        assert_eq!(leaks[2].tokens, vec!["gwern"]);
    }

    #[test]
    fn real_test_module_remains_exempt() {
        let mut leaks = Vec::new();
        scan_file(
            "crates/dli-world/src/example.rs",
            "#[cfg(test)]\nmod tests {\n  const EXAMPLE: &str = \"mara\";\n}\n",
            &mut leaks,
        );
        assert!(leaks.is_empty());
    }
}
