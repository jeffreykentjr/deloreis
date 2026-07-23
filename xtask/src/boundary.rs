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
    let mut in_block_comment = false;

    for (index, line) in text.lines().enumerate() {
        let started_in_block = in_block_comment;
        let trimmed = line.trim_start();
        // F5S-4: a comment (// or /// or //!) mentioning `#[cfg(test)]` must not
        // set the flag — it is documentation or annotation, not an actual attribute.
        if trimmed.contains("#[cfg(test)]") && !trimmed.starts_with("//") {
            pending_cfg = true;
        }

        let delta = brace_delta(line, &mut in_block_comment);

        // Does this line open the pending `#[cfg(test)]` block?
        let opens_test_block = pending_cfg && delta > 0;
        if opens_test_block {
            test_close_depth = Some(depth);
            pending_cfg = false;
        } else if pending_cfg && delta <= 0 && trimmed.trim_end().ends_with(';') {
            // A `#[cfg(test)]` on a non-block item (e.g. a `use`): not a region.
            pending_cfg = false;
        }

        let in_test = test_close_depth.is_some();
        let is_comment_line = started_in_block || trimmed.starts_with("//");
        let exempt = line.contains("tpth-exempt");

        if !in_test && !is_comment_line && !exempt {
            let hits = line_tokens(line);
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

        depth += delta;
        if let Some(close) = test_close_depth
            && depth <= close
        {
            test_close_depth = None;
        }
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

/// Net `{`/`}` count on a line, ignoring braces inside strings, char/byte
/// literals, and comments. `in_block_comment` carries `/* */` state across lines.
fn brace_delta(line: &str, in_block_comment: &mut bool) -> i64 {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut delta: i64 = 0;
    let mut i = 0;

    while i < len {
        if *in_block_comment {
            if bytes[i] == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                *in_block_comment = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        match bytes[i] {
            b'/' if i + 1 < len && bytes[i + 1] == b'/' => break, // line comment
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                *in_block_comment = true;
                i += 2;
            }
            b'"' => i = skip_string(bytes, i + 1),
            b'\'' => i = skip_char_or_lifetime(bytes, i),
            b'{' => {
                delta += 1;
                i += 1;
            }
            b'}' => {
                delta -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    delta
}

/// Advance past a double-quoted string body, returning the index after the
/// closing quote (or end of line).
fn skip_string(bytes: &[u8], mut i: usize) -> usize {
    let len = bytes.len();
    while i < len {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    len
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
