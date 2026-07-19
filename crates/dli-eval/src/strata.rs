//! Stratification of the rule set.
//!
//! Intensional predicates are partitioned into evaluation strata by
//! strongly-connected component: mutually recursive predicates share a stratum,
//! and a stratum depends only on earlier ones. With no negation in the current
//! surface there are no negative dependencies, so every program stratifies;
//! the SCC machinery is what will localise a future negative cycle to the exact
//! atoms involved.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::EvalError;
use crate::world::Rule;

/// Predicates partitioned into evaluation strata, lowest dependency first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Strata {
    /// Each level names the predicates evaluated together; earlier levels are
    /// saturated before later ones consume them.
    pub levels: Vec<Vec<String>>,
}

/// Partition a rule set's head predicates into dependency-ordered strata.
pub fn stratify_rules(rules: &[Rule]) -> Result<Strata, EvalError> {
    let mut preds: Vec<String> = rules.iter().map(|r| r.head.pred.text.clone()).collect();
    preds.sort();
    preds.dedup();
    let index: BTreeMap<&str, usize> = preds
        .iter()
        .enumerate()
        .map(|(i, p)| (p.as_str(), i))
        .collect();
    let n = preds.len();

    let mut adjacency: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); n];
    for rule in rules {
        let head = index[rule.head.pred.text.as_str()];
        for body_atom in &rule.body {
            if let Some(&dep) = index.get(body_atom.pred.text.as_str()) {
                adjacency[head].insert(dep);
            }
        }
    }

    let components = tarjan(n, &adjacency);
    let mut component_of = vec![0usize; n];
    for (id, component) in components.iter().enumerate() {
        for &node in component {
            component_of[node] = id;
        }
    }

    let mut condensation: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); components.len()];
    for (node, deps) in adjacency.iter().enumerate() {
        for &dep in deps {
            if component_of[node] != component_of[dep] {
                condensation[component_of[node]].insert(component_of[dep]);
            }
        }
    }

    let mut depth = vec![None; components.len()];
    for c in 0..components.len() {
        component_depth(c, &condensation, &mut depth);
    }

    let names: Vec<Vec<String>> = components
        .iter()
        .map(|component| {
            let mut level: Vec<String> =
                component.iter().map(|&node| preds[node].clone()).collect();
            level.sort();
            level
        })
        .collect();

    let mut order: Vec<usize> = (0..components.len()).collect();
    order.sort_by(|&a, &b| {
        depth[a]
            .cmp(&depth[b])
            .then_with(|| names[a].cmp(&names[b]))
    });
    let levels = order.into_iter().map(|c| names[c].clone()).collect();
    Ok(Strata { levels })
}

/// The dependency depth of a condensation node (dependencies are strictly
/// shallower); memoised over the acyclic condensation.
fn component_depth(
    component: usize,
    condensation: &[BTreeSet<usize>],
    depth: &mut [Option<usize>],
) -> usize {
    if let Some(known) = depth[component] {
        return known;
    }
    let mut deepest = 0;
    for &dep in &condensation[component] {
        deepest = deepest.max(1 + component_depth(dep, condensation, depth));
    }
    depth[component] = Some(deepest);
    deepest
}

/// Tarjan's strongly-connected-components, returned as node-index groups.
fn tarjan(n: usize, adjacency: &[BTreeSet<usize>]) -> Vec<Vec<usize>> {
    let mut state = Tarjan {
        adjacency,
        index: vec![None; n],
        low: vec![0; n],
        on_stack: vec![false; n],
        stack: Vec::new(),
        next: 0,
        components: Vec::new(),
    };
    for node in 0..n {
        if state.index[node].is_none() {
            state.connect(node);
        }
    }
    state.components
}

/// Mutable state for one Tarjan traversal.
struct Tarjan<'a> {
    adjacency: &'a [BTreeSet<usize>],
    index: Vec<Option<usize>>,
    low: Vec<usize>,
    on_stack: Vec<bool>,
    stack: Vec<usize>,
    next: usize,
    components: Vec<Vec<usize>>,
}

impl Tarjan<'_> {
    fn connect(&mut self, node: usize) {
        self.index[node] = Some(self.next);
        self.low[node] = self.next;
        self.next += 1;
        self.stack.push(node);
        self.on_stack[node] = true;

        for &next in &self.adjacency[node].clone() {
            match self.index[next] {
                None => {
                    self.connect(next);
                    self.low[node] = self.low[node].min(self.low[next]);
                }
                Some(next_index) if self.on_stack[next] => {
                    self.low[node] = self.low[node].min(next_index);
                }
                Some(_) => {}
            }
        }

        if Some(self.low[node]) == self.index[node] {
            let mut component = Vec::new();
            while let Some(member) = self.stack.pop() {
                self.on_stack[member] = false;
                component.push(member);
                if member == node {
                    break;
                }
            }
            self.components.push(component);
        }
    }
}
