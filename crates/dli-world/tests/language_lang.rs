#![allow(clippy::unwrap_used, clippy::expect_used, clippy::format_push_string)]
//! Language-layer integration and parity oracles (`LANG-1`/`LANG-2`).
//!
//! Offline and deterministic: the parity tests read the vendored corpus
//! (`worlds/tpth/vendor/…/registers.yaml`, `ratified_names.yaml`) from the repo,
//! never the network. They prove three things the language layer must hold:
//!
//! 1. **Port 1:1** — every vendored register, expressed as `.dli` and compiled
//!    through the kernel, reproduces the source's register count, per-register
//!    morpheme count, role set, and phonotactic frame.
//! 2. **The canonical example checks clean** — `examples/cymric-fringe.dli`
//!    compiles, its ratified names pass every derivation obligation, and the
//!    proposal bundle promotes under its `ratify`.
//! 3. **The vet split holds** — the ratified name-space is internally consistent;
//!    killed *factory-half* candidates (a franchise collision, an anachronism, an
//!    audio homophone) are kernel-clean; and synthesized *world-true* collisions
//!    (a duplicate, a near-homophone) are flagged by the kernel.

use std::path::PathBuf;

use dli_ast::decl::Document;
use dli_syntax::parse_document;
use dli_world::language::{self, LanguageModel, ratified_names, vet_candidate};

use serde_yaml_ng::Value;

/// A repo-root-relative path to a vendored or example asset.
fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

fn parse(src: &str) -> Document {
    parse_document(src).0
}

const REGISTERS_YAML: &str =
    "worlds/tpth/vendor/fantasy-40f6679/src/fantasy/naming/data/registers.yaml";
const RATIFIED_YAML: &str =
    "worlds/tpth/vendor/fantasy-40f6679/src/fantasy/naming/data/ratified_names.yaml";

fn strs(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_sequence)
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(str::to_lowercase))
                .collect()
        })
        .unwrap_or_default()
}

/// Slug a vendored `form` into a legal lowercase bareword morpheme surface.
fn slug(form: &str) -> String {
    let mut out = String::new();
    for c in form.chars() {
        if c.is_ascii_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_owned()
}

/// Express the vendored register data as a `.dli` pack — the port under test.
fn ported_registers_dli(registers: &Value) -> String {
    let mut src = String::from("scope language\n");
    for (id, reg) in registers.as_mapping().expect("registers is a mapping") {
        let id = id.as_str().expect("register id");
        let label = reg.get("label").and_then(Value::as_str).unwrap_or(id);
        src.push_str(&format!("dialect {id} \"{label}\" ratified D-PORT {{\n"));

        let ph = reg.get("phonotactics").expect("phonotactics");
        push_quoted_list(&mut src, "vowels", &strs(ph.get("vowels")));
        push_quoted_list(&mut src, "endings", &strs(ph.get("endings")));
        push_quoted_list(&mut src, "foreign", &strs(ph.get("foreign")));
        if let Some(max) = ph.get("max_cluster").and_then(Value::as_u64) {
            src.push_str(&format!("cluster-max {max}\n"));
        }
        if let Some(stress) = ph.get("stress").and_then(Value::as_str) {
            src.push_str(&format!("stress {stress}\n"));
        }

        if let Some(patterns) = reg.get("patterns").and_then(Value::as_mapping) {
            for (kind, alts) in patterns {
                let kind = kind.as_str().expect("pattern kind");
                let alts: Vec<String> = alts
                    .as_sequence()
                    .expect("pattern alternatives")
                    .iter()
                    .map(|alt| strs(alt.get("slots")).join(" "))
                    .filter(|s| !s.is_empty())
                    .collect();
                if !alts.is_empty() {
                    src.push_str(&format!("name-kind {kind} = {}\n", alts.join(" | ")));
                }
            }
        }

        for el in reg
            .get("elements")
            .and_then(Value::as_sequence)
            .unwrap_or(&vec![])
        {
            let form = el.get("form").and_then(Value::as_str).unwrap_or_default();
            let role = el.get("role").and_then(Value::as_str).unwrap_or_default();
            if form.is_empty() || role.is_empty() {
                continue;
            }
            let gender = el
                .get("gender")
                .and_then(Value::as_str)
                .map(|g| format!(" gender {g}"))
                .unwrap_or_default();
            src.push_str(&format!(
                "name-part {} \"g\" role {role}{gender} ratified D-PORT\n",
                slug(form)
            ));
        }
        src.push_str("}\n");
    }
    src
}

fn push_quoted_list(src: &mut String, keyword: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    let quoted: Vec<String> = items.iter().map(|s| format!("\"{s}\"")).collect();
    src.push_str(&format!("{keyword} {}\n", quoted.join(" ")));
}

#[test]
fn registers_port_one_to_one() {
    let yaml: Value = serde_yaml_ng::from_str(&read(REGISTERS_YAML)).expect("parse registers.yaml");
    let registers = yaml.get("registers").expect("registers key");

    let dli = ported_registers_dli(registers);
    let model = LanguageModel::from_documents(&[parse(&dli)]).model;

    let expected = registers.as_mapping().unwrap();
    assert_eq!(
        model.registers.len(),
        expected.len(),
        "every vendored register ports"
    );

    for (id, reg) in expected {
        let id = id.as_str().unwrap();
        let compiled = model
            .register(id)
            .unwrap_or_else(|| panic!("register {id} compiled"));

        let elements = reg.get("elements").and_then(Value::as_sequence).unwrap();
        assert_eq!(
            compiled.morphemes.len(),
            elements.len(),
            "register {id}: morpheme count is 1:1 with vendored elements"
        );

        let yaml_roles: std::collections::BTreeSet<String> = elements
            .iter()
            .filter_map(|el| el.get("role").and_then(Value::as_str).map(str::to_owned))
            .collect();
        let compiled_roles: std::collections::BTreeSet<String> =
            compiled.morphemes.iter().map(|m| m.role.clone()).collect();
        assert_eq!(compiled_roles, yaml_roles, "register {id}: role set ports");

        let ph = reg.get("phonotactics").unwrap();
        assert_eq!(
            compiled.phonotactics.vowels,
            strs(ph.get("vowels")),
            "register {id}: vowel inventory ports"
        );
        assert_eq!(
            compiled.phonotactics.cluster_max as u64,
            ph.get("max_cluster").and_then(Value::as_u64).unwrap(),
            "register {id}: cluster ceiling ports"
        );

        let pattern_count = reg
            .get("patterns")
            .and_then(Value::as_mapping)
            .map_or(0, serde_yaml_ng::Mapping::len);
        assert_eq!(
            compiled.name_kinds.len(),
            pattern_count,
            "register {id}: name-kind grammars port"
        );
    }
}

#[test]
fn cymric_example_compiles_and_checks_clean() {
    let doc = parse(&read("worlds/tpth/packs/cymric-fringe.dli"));
    let out = LanguageModel::from_documents(&[doc]);
    let model = out.model;

    assert!(
        model.register("cymric-fringe").is_some(),
        "register compiled"
    );
    let names: Vec<&str> = model.names.iter().map(|n| n.name.as_str()).collect();
    for expected in ["enfys", "corswern", "gwenlys"] {
        assert!(names.contains(&expected), "name {expected} compiled");
    }

    let findings = language::check_language(&model);
    assert!(
        findings.is_empty(),
        "the canonical example is derivation-clean, got: {:?}",
        findings.iter().map(LangFindingMsg::msg).collect::<Vec<_>>()
    );

    // enfys and corswern are ratified; gwenlys is a proposal promoted by the
    // bundle's `ratify D-0195 { h-cymric-loans := true }` (05 §5).
    let view: Vec<String> = ratified_names(&model).into_iter().map(|r| r.name).collect();
    for expected in ["Enfys", "Corswern", "Gwenlys"] {
        assert!(
            view.contains(&expected.to_owned()),
            "ratified-names view holds {expected}, got {view:?}"
        );
    }

    // The world-check aggregator sees the same clean verdict.
    assert!(language::check(&model).is_empty());
}

/// A tiny extension trait so the failure message can render findings.
trait LangFindingMsg {
    fn msg(&self) -> String;
}
impl LangFindingMsg for language::LangFinding {
    fn msg(&self) -> String {
        format!("[{}] {}", self.code(), self.message())
    }
}

/// Build a model from the vendored registers plus a handful of composed names,
/// so register-fit runs against real register data.
fn model_with_composed_names() -> LanguageModel {
    let yaml: Value = serde_yaml_ng::from_str(&read(REGISTERS_YAML)).expect("parse registers.yaml");
    let mut dli = ported_registers_dli(yaml.get("registers").unwrap());
    // Real ratified compositions (canon/geography/regions.md), plus one killed
    // franchise-collision candidate that is nonetheless register-valid.
    dli.push_str(
        "name whitecombe \"Whitecombe\" place in chalk = place-qualifier white + place-generic combe ratified D-0111\n\
         name swartby \"Swartby\" place in crags = place-qualifier swart + place-generic by ratified D-0111\n\
         name halspetra \"Halspetra\" place in fringes = place-qualifier hals + place-generic petra ratified D-0111\n\
         name corsbach \"Corsbach\" place in lowlands = place-generic cors + place-qualifier bach ratified D-0111\n\
         name irondale \"Irondale\" place in crags = place-qualifier iron + place-generic dale proposed agent:run assumes h-iron\n",
    );
    LanguageModel::from_documents(&[parse(&dli)]).model
}

#[test]
fn composed_names_pass_register_fit_on_real_data() {
    let model = model_with_composed_names();
    let findings = language::check_language(&model);
    assert!(
        findings.is_empty(),
        "real ratified compositions and a register-valid candidate all fit: {:?}",
        findings.iter().map(LangFindingMsg::msg).collect::<Vec<_>>()
    );
    // Irondale is register-valid: the kernel is clean. Its Game-of-Thrones
    // collision is a factory-side screen (05 §3), not a kernel finding.
    let irondale = model.names.iter().find(|n| n.name == "irondale").unwrap();
    assert!(irondale.composition.len() == 2);
}

#[test]
fn off_register_composition_fails_the_kernel() {
    // A place name carrying a register-foreign cluster ('z' is foreign to the
    // fringe tongue) and a wrong-role morpheme both fail register-fit.
    let src = "scope language\n\
        dialect cymric-fringe \"fringe\" ratified D-1 {\n\
        vowels a e i o u w y\n\
        foreign qu x z tion\n\
        cluster-max 2\n\
        name-kind place = place-qualifier place-generic\n\
        name-part cors \"bog\" role place-qualifier ratified D-1\n\
        name-part wern \"marsh\" role place-generic ratified D-1\n\
        name-part zwed \"z-loan\" role place-generic ratified D-1\n\
        }\n\
        name bad \"Bad\" place in cymric-fringe = place-qualifier cors + place-generic zwed ratified D-1\n\
        name wrong \"Wrong\" place in cymric-fringe = place-generic cors + place-qualifier wern ratified D-1\n";
    let model = LanguageModel::from_documents(&[parse(src)]).model;
    let codes: Vec<&str> = language::check_language(&model)
        .iter()
        .map(language::LangFinding::code)
        .collect();
    assert!(
        codes.contains(&"name.phonotactics"),
        "the foreign 'z' cluster is caught: {codes:?}"
    );
    assert!(
        codes.contains(&"name.slot-sequence") || codes.contains(&"name.role-membership"),
        "the wrong slot order is caught: {codes:?}"
    );
}

/// Build a name-space model whose certain layer is the vendored ratified names.
fn ratified_name_space() -> LanguageModel {
    let yaml: Value = serde_yaml_ng::from_str(&read(RATIFIED_YAML)).expect("parse ratified_names");
    let mut dli = String::from("scope language\n");
    for entry in yaml.get("names").and_then(Value::as_sequence).unwrap() {
        let name = entry.get("name").and_then(Value::as_str).unwrap();
        let kind = entry
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("concept");
        let decision = entry
            .get("decision")
            .and_then(Value::as_str)
            .unwrap_or("D-0000");
        dli.push_str(&format!(
            "name {} \"{name}\" {kind} in none ratified {decision}\n",
            slug(name)
        ));
    }
    LanguageModel::from_documents(&[parse(&dli)]).model
}

#[test]
fn shadow_vet_ratified_names_all_pass() {
    let model = ratified_name_space();
    // Every ratified name is in the certain layer.
    assert_eq!(
        ratified_names(&model).len(),
        model.names.len(),
        "all vendored ratified names are assertable"
    );
    // The ratified set is internally consistent: no uniqueness or aural clash,
    // nothing banned. This is "run the kernel checks over the ratified names —
    // all must pass" (LANG-2 oracle).
    let findings = language::check_language(&model);
    assert!(
        findings.is_empty(),
        "the ratified name-space is derivation-clean, got: {:?}",
        findings.iter().map(LangFindingMsg::msg).collect::<Vec<_>>()
    );
}

#[test]
fn shadow_vet_splits_factory_from_world_true() {
    let model = ratified_name_space();

    // Factory-half kills — a franchise collision (D-0101), an anachronism/Tiffany
    // (D-0129), an audio homophone (D-0108). Their ground truth is a fact about
    // OUR world; the kernel half is clean (05 §3).
    for factory_clean in ["Irondale", "Marcia", "Wynne"] {
        let findings = vet_candidate(&model, factory_clean);
        assert!(
            findings.is_empty(),
            "{factory_clean} is a factory-side kill; the kernel is clean, got: {:?}",
            findings.iter().map(LangFindingMsg::msg).collect::<Vec<_>>()
        );
    }

    // World-true failures — a duplicate of a settled name, and a near-homophone of
    // one. Their ground truth is a fact about the STORY's world; the kernel flags
    // them (05 §3).
    let dup = vet_candidate(&model, "Gwern");
    assert!(
        dup.iter().any(|f| f.code() == "name.uniqueness"),
        "a duplicate of settled 'Gwern' is a uniqueness clash: {:?}",
        dup.iter().map(LangFindingMsg::msg).collect::<Vec<_>>()
    );

    let near = vet_candidate(&model, "Trefpenn");
    assert!(
        near.iter().any(|f| f.code() == "name.aural-clash"),
        "'Trefpenn' sounds too like settled 'Trefpen': {:?}",
        near.iter().map(LangFindingMsg::msg).collect::<Vec<_>>()
    );
}
