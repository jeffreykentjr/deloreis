//! Advisory checks for entity references.

use std::collections::{BTreeMap, BTreeSet};

use dli_ast::term::Term;

use super::{CompiledWorld, WorldFinding};

/// Warn about unknown names in positions established as entity references,
/// and near-misses in subjects. Abstract subjects without evidence of a typo
/// remain valid. The result is advisory and never blocks materialization.
pub(super) fn run(world: &CompiledWorld) -> Vec<WorldFinding> {
    let sorts: BTreeMap<&str, &str> = world
        .model
        .entities
        .iter()
        .filter_map(|entity| Some((entity.name.as_str(), entity.sort.as_deref()?)))
        .collect();
    let mut names_by_pack: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for atom in &world.model.atoms {
        if let [Term::Sym(id)] = atom.atom.args.as_slice()
            && sorts.get(id.text.as_str()) == Some(&atom.atom.pred.text.as_str())
        {
            names_by_pack
                .entry(&atom.pack)
                .or_default()
                .insert(&id.text);
        }
    }
    let mut positions = BTreeSet::new();
    for atom in &world.model.atoms {
        let Some(names) = names_by_pack.get(atom.pack.as_str()) else {
            continue;
        };
        for (index, term) in atom.atom.args.iter().enumerate() {
            if let Term::Sym(id) = term
                && names.contains(id.text.as_str())
            {
                positions.insert((atom.pack.as_str(), atom.atom.pred.text.as_str(), index));
            }
        }
    }
    let mut findings = Vec::new();
    let mut seen = BTreeSet::new();
    for (atom_index, atom) in world.model.atoms.iter().enumerate() {
        let Some(names) = names_by_pack.get(atom.pack.as_str()) else {
            continue;
        };
        for (index, term) in atom.atom.args.iter().enumerate() {
            let Term::Sym(id) = term else { continue };
            // An undotted state lowers its value into argument zero; it is not
            // a subject and must not be treated as one merely by position.
            if index == 0
                && world.model.temporal_atom_facts.contains_key(&atom_index)
                && !(id.span.end > id.span.start
                    && id.span == atom.atom.pred.span
                    && id.span.end.saturating_sub(id.span.start) as usize > id.text.len())
            {
                continue;
            }
            if names.contains(id.text.as_str()) {
                continue;
            }
            let mut matches: Vec<_> = names
                .iter()
                .filter_map(|name| {
                    if id.text.chars().count().abs_diff(name.chars().count()) > 2 {
                        return None;
                    }
                    let distance = edit_distance(&id.text, name);
                    (distance > 0 && distance <= 2).then_some((distance, *name))
                })
                .collect();
            matches.sort_unstable();
            let candidates: Vec<&str> = matches.iter().take(5).map(|(_, name)| *name).collect();
            if candidates.is_empty()
                && !positions.contains(&(atom.pack.as_str(), atom.atom.pred.text.as_str(), index))
            {
                continue;
            }
            if index != 0
                && !positions.contains(&(atom.pack.as_str(), atom.atom.pred.text.as_str(), index))
            {
                continue;
            }
            let key = (id.text.clone(), id.span.file.0, id.span.start, id.span.end);
            if !seen.insert(key) {
                continue;
            }
            let message = if let Some(candidate) = candidates.first() {
                format!("`{}` is not declared; did you mean `{candidate}`?", id.text)
            } else {
                format!("`{}` is not declared", id.text)
            };
            let width = id.span.end.saturating_sub(id.span.start) as usize;
            let span = if width == id.text.len() && width > 0 {
                Some(id.span)
            } else if index == 0
                && id.span == atom.atom.pred.span
                && width >= id.text.len() + 1 + atom.atom.pred.text.len()
            {
                u32::try_from(id.text.len()).ok().and_then(|length| {
                    id.span
                        .start
                        .checked_add(length)
                        .map(|end| dli_ast::span::Span { end, ..id.span })
                })
            } else {
                None
            };
            let finding = WorldFinding::warn("entity.undeclared", message).with_data(
                serde_json::json!({"entity": id.text, "candidates": candidates, "span": span}),
            );
            findings.push(match span {
                Some(span) => finding.with_span(span),
                None => finding,
            });
        }
    }
    findings
}

/// Levenshtein distance capped at three. Only the diagonal band within two
/// edits is relevant to a suggestion, so long identifiers take linear work.
fn edit_distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).map(|n| n.min(3)).collect();
    let mut current = vec![3; right.len() + 1];
    for (offset, a) in left.chars().enumerate() {
        let i = offset + 1;
        let first = i.saturating_sub(2).max(1);
        let last = (i + 2).min(right.len());
        current[0] = i.min(3);
        if first > 1 && first - 1 < current.len() {
            current[first - 1] = 3;
        }
        if last + 1 < current.len() {
            current[last + 1] = 3;
        }
        let mut best = current[0];
        if first <= last {
            for j in first..=last {
                current[j] = (previous[j] + 1)
                    .min(current[j - 1] + 1)
                    .min(previous[j - 1] + usize::from(a != right[j - 1]))
                    .min(3);
                best = best.min(current[j]);
            }
        }
        if best > 2 {
            return 3;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::edit_distance;

    #[test]
    fn bounded_distance_matches_full_distance_on_short_strings() {
        fn words(prefix: &str, remaining: usize, out: &mut Vec<String>) {
            out.push(prefix.to_owned());
            if remaining > 0 {
                for ch in ['a', 'b'] {
                    let mut next = prefix.to_owned();
                    next.push(ch);
                    words(&next, remaining - 1, out);
                }
            }
        }
        fn full(left: &str, right: &str) -> usize {
            let mut previous: Vec<usize> = (0..=right.len()).collect();
            for (i, a) in left.bytes().enumerate() {
                let mut current = vec![i + 1];
                for (j, b) in right.bytes().enumerate() {
                    current.push(
                        (previous[j + 1] + 1)
                            .min(current[j] + 1)
                            .min(previous[j] + usize::from(a != b)),
                    );
                }
                previous = current;
            }
            previous[right.len()]
        }
        let mut corpus = Vec::new();
        words("", 5, &mut corpus);
        for left in &corpus {
            for right in &corpus {
                assert_eq!(
                    edit_distance(left, right),
                    full(left, right).min(3),
                    "{left:?}, {right:?}"
                );
            }
        }
    }
}
