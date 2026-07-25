//! Machine checks for the decision ledger and PLAN node-state matrix.

use std::collections::{BTreeMap, BTreeSet};
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
    let corrected_dates = corrected_dates(ledger)?;
    let mut entries = Vec::new();
    for (line_index, line) in ledger.lines().enumerate() {
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
            .skip(start + 1)
            .take(end - start - 1)
            .collect();
        if !section.iter().any(|line| valid_date_line(line)) && !corrected_dates.contains_key(&id) {
            bail!("DL-{id:04} has no valid `*Date: YYYY-MM-DD*` line");
        }
        if !section.iter().any(|line| line.starts_with("**Ruling")) {
            bail!("DL-{id:04} has no ruling");
        }
    }
    Ok(())
}

fn corrected_dates(ledger: &str) -> Result<BTreeMap<u32, String>> {
    let mut corrections = BTreeMap::new();
    for line in ledger.lines() {
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
        let date_line = format!("*Date: {raw_date}*");
        if !valid_date_line(&date_line) {
            bail!("{LEDGER} has an invalid corrected date for DL-{id:04}");
        }
        if corrections.insert(id, raw_date.to_owned()).is_some() {
            bail!("{LEDGER} corrects the date for DL-{id:04} more than once");
        }
    }
    Ok(corrections)
}

fn valid_date_line(line: &str) -> bool {
    let Some(date) = line
        .strip_prefix("*Date: ")
        .and_then(|value| value.strip_suffix('*'))
    else {
        return false;
    };
    date.len() == 10
        && date.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 => byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

fn validate_append_only(root: &Path, current: &[u8]) -> Result<()> {
    let Some(head) = git_file(root, "HEAD", LEDGER)? else {
        return Ok(());
    };
    let baseline = if current == head {
        git_file(root, "HEAD^", LEDGER)?
    } else {
        Some(head)
    };
    if let Some(previous) = baseline
        && !current.starts_with(&previous)
    {
        bail!(
            "{LEDGER} is not append-only: bytes already present in the comparison revision changed"
        );
    }
    Ok(())
}

fn git_file(root: &Path, revision: &str, path: &str) -> Result<Option<Vec<u8>>> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["show", &format!("{revision}:{path}")])
        .output()
        .context("could not invoke git for the ledger append-only check")?;
    if output.status.success() {
        Ok(Some(output.stdout))
    } else {
        Ok(None)
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
    use super::{valid_date_line, validate_ledger, validate_plan};

    #[test]
    fn date_line_is_strict() {
        assert!(valid_date_line("*Date: 2026-07-25*"));
        assert!(!valid_date_line("*Date: 2026-7-25*"));
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
