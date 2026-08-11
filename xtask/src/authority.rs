//! Machine checks for the decision ledger and PLAN node-state matrix.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

const LEDGER: &str = "docs/decisions.md";
const PLAN: &str = "docs/PLAN.md";
const STATES: [&str; 4] = ["done", "open", "deferred", "external-blocked"];

/// Validate the authority documents and, when Git history is available, the
/// append-only ledger invariant.
pub fn run_authority(root: &Path) -> Result<()> {
    let ledger = fs::read_to_string(root.join(LEDGER))
        .with_context(|| format!("could not read {LEDGER}"))?;
    validate_ledger(&ledger)?;
    validate_append_only(root, ledger.as_bytes())?;

    let plan =
        fs::read_to_string(root.join(PLAN)).with_context(|| format!("could not read {PLAN}"))?;
    validate_plan(&plan)?;

    println!("authority: ledger structure, append-only history, and PLAN states are valid");
    Ok(())
}

fn validate_ledger(ledger: &str) -> Result<()> {
    let active = active_markdown_lines(ledger);
    let mut entries = Vec::new();
    for (line_index, line) in ledger
        .lines()
        .enumerate()
        .filter(|(index, _)| active[*index])
    {
        let Some(rest) = line.strip_prefix("## DL-") else {
            continue;
        };
        let Some(raw_id) = rest.split_once(' ').map(|(id, _)| id) else {
            bail!(
                "{}:{} has a malformed DL entry heading",
                LEDGER,
                line_index + 1
            );
        };
        let id = raw_id
            .parse::<u32>()
            .with_context(|| format!("{}:{} has a non-numeric DL id", LEDGER, line_index + 1))?;
        entries.push((id, line_index));
    }
    if entries.is_empty() {
        bail!("{LEDGER} contains no decision entries");
    }
    let corrected_dates = corrected_dates(ledger, &entries, &active)?;

    for (expected, (id, start)) in (1_u32..).zip(entries.iter().copied()) {
        if id != expected {
            bail!("{LEDGER} has DL-{id:04} where DL-{expected:04} is required");
        }
        let end = entries
            .iter()
            .find_map(|(_, candidate)| (*candidate > start).then_some(*candidate))
            .unwrap_or_else(|| ledger.lines().count());
        let section: Vec<&str> = ledger
            .lines()
            .enumerate()
            .skip(start + 1)
            .take(end - start - 1)
            .filter(|(index, _)| active[*index])
            .map(|(_, line)| line)
            .collect();
        if !section.iter().any(|line| valid_date_line(line)) && !corrected_dates.contains_key(&id) {
            bail!("DL-{id:04} has no valid `*Date: YYYY-MM-DD*` line");
        }
        if !section.iter().any(|line| valid_ruling_line(line)) {
            bail!("DL-{id:04} has no ruling");
        }
    }
    Ok(())
}

fn corrected_dates(
    ledger: &str,
    entries: &[(u32, usize)],
    active: &[bool],
) -> Result<BTreeMap<u32, String>> {
    let mut corrections = BTreeMap::new();
    let lines: Vec<&str> = ledger.lines().collect();
    for (entry_index, (corrector, start)) in entries.iter().copied().enumerate() {
        let end = entries
            .get(entry_index + 1)
            .map_or(lines.len(), |(_, line)| *line);
        for (line_index, line) in lines.iter().enumerate().take(end).skip(start + 1) {
            if !active[line_index] {
                continue;
            }
            let Some(rest) = line.strip_prefix("*Corrected date: DL-") else {
                continue;
            };
            let Some((raw_id, raw_date)) = rest
                .strip_suffix('*')
                .and_then(|value| value.split_once(" = "))
            else {
                bail!("{LEDGER} has a malformed corrected-date line");
            };
            let id = raw_id
                .parse::<u32>()
                .context("ledger corrected-date line has a non-numeric id")?;
            if id == 0 || id >= corrector {
                bail!(
                    "DL-{corrector:04} cannot correct its own or a later entry's date (DL-{id:04})"
                );
            }
            let date_line = format!("*Date: {raw_date}*");
            if !valid_date_line(&date_line) {
                bail!("{LEDGER} has an invalid corrected date for DL-{id:04}");
            }
            if corrections.insert(id, raw_date.to_owned()).is_some() {
                bail!("{LEDGER} corrects the date for DL-{id:04} more than once");
            }
        }
    }
    Ok(corrections)
}

fn active_markdown_lines(markdown: &str) -> Vec<bool> {
    let mut active = Vec::new();
    let mut fence: Option<(u8, usize)> = None;
    for line in markdown.lines() {
        let indent = line
            .as_bytes()
            .iter()
            .take(3)
            .take_while(|byte| **byte == b' ')
            .count();
        let candidate = &line[indent..];
        let marker = candidate.as_bytes().first().copied();
        let run = marker.map_or(0, |byte| {
            candidate
                .as_bytes()
                .iter()
                .take_while(|candidate| **candidate == byte)
                .count()
        });

        if let Some((fence_marker, minimum_run)) = fence {
            active.push(false);
            if marker == Some(fence_marker)
                && run >= minimum_run
                && candidate[run..].trim().is_empty()
            {
                fence = None;
            }
        } else if let Some(marker @ (b'`' | b'~')) = marker
            && run >= 3
        {
            active.push(false);
            fence = Some((marker, run));
        } else {
            active.push(true);
        }
    }
    active
}

fn valid_ruling_line(line: &str) -> bool {
    if line
        .strip_prefix("**Ruling.**")
        .is_some_and(marker_is_followed_by_prose)
    {
        return true;
    }
    if let Some(rest) = line.strip_prefix("**Ruling** (") {
        return rest.split_once("):").is_some_and(|(detail, prose)| {
            !detail.is_empty() && marker_is_followed_by_prose(prose)
        });
    }
    let Some(rest) = line.strip_prefix("**Ruling (") else {
        return false;
    };
    let Some((number, suffix)) = rest.split_once(')') else {
        return false;
    };
    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    if let Some(prose) = suffix.strip_prefix(":**") {
        return marker_is_followed_by_prose(prose);
    }
    suffix
        .strip_prefix(" — ")
        .and_then(|detail| detail.split_once(":**"))
        .is_some_and(|(detail, prose)| !detail.is_empty() && marker_is_followed_by_prose(prose))
}

fn marker_is_followed_by_prose(rest: &str) -> bool {
    rest.is_empty() || rest.starts_with(char::is_whitespace)
}

fn valid_date_line(line: &str) -> bool {
    let Some(date) = line
        .strip_prefix("*Date: ")
        .and_then(|value| value.strip_suffix('*'))
    else {
        return false;
    };
    if date.len() != 10
        || !date.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 => byte == b'-',
            _ => byte.is_ascii_digit(),
        })
    {
        return false;
    }
    let Ok(year) = date[0..4].parse::<u32>() else {
        return false;
    };
    let Ok(month) = date[5..7].parse::<u32>() else {
        return false;
    };
    let Ok(day) = date[8..10].parse::<u32>() else {
        return false;
    };
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    year > 0 && (1..=days).contains(&day)
}

fn validate_append_only(root: &Path, current: &[u8]) -> Result<()> {
    let base_ref = baseline_ref(env::var("GITHUB_BASE_REF").ok());
    let merge_base = git_merge_base(root, &base_ref)?;
    let baseline = git_file(root, &merge_base, LEDGER)?;
    if !current.starts_with(&baseline) {
        bail!(
            "{LEDGER} is not append-only: bytes already present in the comparison revision changed"
        );
    }
    Ok(())
}

fn baseline_ref(github_base_ref: Option<String>) -> String {
    match github_base_ref {
        Some(branch) if !branch.is_empty() => format!("origin/{branch}"),
        _ => "origin/main".to_owned(),
    }
}

fn git_merge_base(root: &Path, base_ref: &str) -> Result<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["merge-base", "HEAD", base_ref])
        .output()
        .with_context(|| format!("could not invoke git to find merge-base with {base_ref}"))?;
    if !output.status.success() {
        bail!(
            "could not read the authority baseline: git merge-base HEAD {base_ref} failed; check that the base ref is available and GITHUB_BASE_REF is not empty"
        );
    }
    let revision = String::from_utf8(output.stdout)
        .context("git merge-base returned non-UTF-8 output")?
        .trim()
        .to_owned();
    if revision.is_empty() {
        bail!("could not read the authority baseline: merge-base was empty");
    }
    Ok(revision)
}

fn git_file(root: &Path, revision: &str, path: &str) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["show", &format!("{revision}:{path}")])
        .output()
        .context("could not invoke git for the ledger append-only check")?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        bail!(
            "could not read the authority baseline `{revision}:{path}`; missing history is a hard failure"
        );
    }
}

fn validate_plan(plan: &str) -> Result<()> {
    let registry = section(plan, "## The node registry", "## The dependency graph")?;
    let mut nodes = BTreeSet::new();
    for line in registry.lines() {
        if let Some(rest) = line.strip_prefix("- **")
            && let Some((id, _)) = rest.split_once("**")
            && !nodes.insert(id.to_owned())
        {
            bail!("{PLAN} node registry repeats `{id}`");
        }
    }
    if nodes.is_empty() {
        bail!("{PLAN} node registry is empty");
    }

    let matrix = section(plan, "## Node status matrix", "## Milestones")?;
    let mut statuses = BTreeMap::new();
    for line in matrix.lines().filter(|line| line.starts_with('|')) {
        let columns: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        if columns.len() != 3 || columns[0] == "Node" || columns[0].bytes().all(|byte| byte == b'-')
        {
            continue;
        }
        let [node, state, evidence] = columns.as_slice() else {
            continue;
        };
        if !STATES.contains(state) {
            bail!("{PLAN} gives `{node}` unknown state `{state}`");
        }
        if evidence.is_empty() || *evidence == "—" {
            bail!("{PLAN} gives `{node}` no evidence or blocking reason");
        }
        if statuses
            .insert((*node).to_owned(), (*state).to_owned())
            .is_some()
        {
            bail!("{PLAN} status matrix repeats `{node}`");
        }
    }

    let listed: BTreeSet<String> = statuses.into_keys().collect();
    let missing: Vec<&String> = nodes.difference(&listed).collect();
    let unknown: Vec<&String> = listed.difference(&nodes).collect();
    if !missing.is_empty() || !unknown.is_empty() {
        bail!("{PLAN} status matrix mismatch; missing={missing:?}, unknown={unknown:?}");
    }
    Ok(())
}

fn section<'a>(text: &'a str, start: &str, end: &str) -> Result<&'a str> {
    let (_, after_start) = text
        .split_once(start)
        .with_context(|| format!("{PLAN} lacks `{start}`"))?;
    let (body, _) = after_start
        .split_once(end)
        .with_context(|| format!("{PLAN} lacks `{end}` after `{start}`"))?;
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::{baseline_ref, valid_date_line, validate_ledger, validate_plan};

    #[test]
    fn baseline_ref_defaults_when_github_base_ref_is_unset() {
        assert_eq!(baseline_ref(None), "origin/main");
    }

    #[test]
    fn baseline_ref_defaults_when_github_base_ref_is_empty() {
        assert_eq!(baseline_ref(Some(String::new())), "origin/main");
    }

    #[test]
    fn baseline_ref_uses_main_branch() {
        assert_eq!(baseline_ref(Some("main".to_owned())), "origin/main");
    }

    #[test]
    fn baseline_ref_uses_feature_branch() {
        assert_eq!(baseline_ref(Some("feature".to_owned())), "origin/feature");
    }

    #[test]
    fn date_line_is_strict() {
        assert!(valid_date_line("*Date: 2026-07-25*"));
        assert!(valid_date_line("*Date: 2024-02-29*"));
        assert!(!valid_date_line("*Date: 2026-7-25*"));
        assert!(!valid_date_line("*Date: 2026-99-99*"));
        assert!(!valid_date_line("*Date: 2026-02-29*"));
        assert!(!valid_date_line("Date: 2026-07-25"));
    }

    #[test]
    fn ledger_requires_dates_and_sequence() {
        let valid = "## DL-0001 — one\n\n*Date: 2026-07-25*\n\n**Ruling.** yes\n";
        validate_ledger(valid).expect("valid ledger");
        let missing_date = "## DL-0001 — one\n\n**Ruling.** yes\n";
        assert!(validate_ledger(missing_date).is_err());
        let gap = "## DL-0002 — two\n\n*Date: 2026-07-25*\n\n**Ruling.** yes\n";
        assert!(validate_ledger(gap).is_err());

        let self_correction = "\
## DL-0001 — one

*Corrected date: DL-0001 = 2026-07-25*

**Ruling.** yes
";
        assert!(validate_ledger(self_correction).is_err());

        let nonexistent_correction = "\
## DL-0001 — one

*Date: 2026-07-25*

*Corrected date: DL-0000 = 2026-07-24*

**Ruling.** yes
";
        assert!(validate_ledger(nonexistent_correction).is_err());

        let later_correction = "\
## DL-0001 — one

**Ruling.** yes

## DL-0002 — correction

*Date: 2026-07-25*

*Corrected date: DL-0001 = 2026-07-24*

**Ruling.** yes
";
        validate_ledger(later_correction).expect("later entry may correct an earlier date");
    }

    #[test]
    fn ledger_ignores_fenced_authority_lookalikes() {
        let fake_entry = "\
```markdown
## DL-0001 — fake
*Date: 2026-07-25*
**Ruling.** fake
```
";
        assert!(validate_ledger(fake_entry).is_err());

        let fake_correction = "\
## DL-0001 — one

**Ruling.** yes

## DL-0002 — correction

*Date: 2026-07-25*

~~~markdown
*Corrected date: DL-0001 = 2026-07-24*
~~~

**Ruling.** yes
";
        assert!(validate_ledger(fake_correction).is_err());

        let fake_ruling = "\
## DL-0001 — one

*Date: 2026-07-25*

````markdown
**Ruling.** fake
````
";
        assert!(validate_ledger(fake_ruling).is_err());
    }

    #[test]
    fn ledger_requires_exact_ruling_markers() {
        let rulingish = "\
## DL-0001 — one

*Date: 2026-07-25*

**Rulingish.** no
";
        assert!(validate_ledger(rulingish).is_err());
        assert!(
            validate_ledger(&rulingish.replace("**Rulingish.** no", "**Ruling** (unterminated"))
                .is_err()
        );

        for ruling in [
            "**Ruling.** yes",
            "**Ruling (1):** yes",
            "**Ruling (2) — clarification:** yes",
            "**Ruling** (supersedes an earlier ruling): yes",
        ] {
            let ledger = format!("## DL-0001 — one\n\n*Date: 2026-07-25*\n\n{ruling}\n");
            validate_ledger(&ledger).expect("accepted ruling marker");
        }
    }

    #[test]
    fn plan_requires_one_state_per_node() {
        let valid = "\
## The node registry
- **A-1** — a
- **B-1** — b
## The dependency graph
graph
## Node status matrix
| Node | State | Evidence |
|---|---|---|
| A-1 | done | `test-a` |
| B-1 | open | tracked work |
## Milestones
later
";
        validate_plan(valid).expect("valid plan");
        assert!(validate_plan(&valid.replace("| B-1 | open | tracked work |\n", "")).is_err());
    }
}
