#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The committed examples and representative expert-tier blocks parse cleanly.
//!
//! This is the in-crate mirror of the repo-wide docs-oracle: it guards the
//! surface forms the spec teaches so a regression is caught in `dli-syntax`'s
//! own suite, not only in the xtask gate.

use std::path::PathBuf;

use dli_syntax::parse_document;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn committed_examples_parse_clean() {
    let examples = repo_root().join("examples");
    let mut saw = false;
    for entry in std::fs::read_dir(&examples).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("dli") {
            continue;
        }
        saw = true;
        let source = std::fs::read_to_string(&path).unwrap();
        let (_doc, diags) = parse_document(&source);
        let errs: Vec<_> = diags.iter().filter(|d| d.class.is_error()).collect();
        assert!(
            errs.is_empty(),
            "{} produced errors: {errs:?}",
            path.display()
        );
    }
    assert!(saw, "expected at least one example .dli file");
}

/// The expert practice block from `docs/spec/04-practices.md`.
const PRACTICE: &str = "\
scope magic

practice spoken-proof  ratified D-0146 D-0148 {
  kind ground = water | field | ore | tide | hearth | boundary
  role witness : entity    requires standing(witness), consent(witness)
  resource debt   : linear conserved
  rule ward-form(ground, fibre)  ratified D-0146 :- ...
  condition seated requires phase(tide) in allowed-hours(rule)
  error fizzle             refines L2 premise-not-available where absent
  cost bodily | cognitive | material | temporal
  }
";

const TIME: &str = "\
scope chronology

era elder-cycles < the-equilibrium < the-first-enclosure
    < the-long-creep < the-beleaguered-present < the-narrators-era

event first-storehouse-lie    during the-first-enclosure   provisional D-0072
before breaking-of-the-stones -> tithe-of-minds-sweeps     provisional D-0072

cycle tide(4) = ebb | slack | flood | high        ratified D-0146
cycle moon(29)                                     ratified D-0148

state gwern.water-status = commons  until enclosure-of-the-parish-water  ratified D-0114
";

const LANGUAGE: &str = "\
scope language

dialect cymric-fringe  \"the fringe tongue\"  provisional D-0181 {
  vowels a e i o u w y
  name-part en   \"spirit\"        role given-proto     ratified D-0134
  }

name enfys \"Enfys\"  person.given in cymric-fringe
  = given-proto en + given-deutero fys
  bearer protagonist   ratified D-0134

term hedgeway \"the hedgeway\"  ratified D-0143
banned machine   D-0056  \"the allegory is structural, never lexical\"

hypothesis h-cymric \"the western fringe speaks a Brythonic-shaped tongue\"

dialect speculative  proposed agent:factory/naming-run-0712  assumes h-cymric {
  inventory placeholder
  }

ratify D-0195 { h-cymric := true }
";

const CRAFT: &str = "\
scope craft

principle tryfail-before-triumph  family plot  mission pass
  source sanderson  cites D-0128
  checkable deterministic -> accomplishment-earned

rule accomplishment-earned  over season  severity error
  : beat $b kind accomplishment magnitude >= major
    requires beat $f  thread($f) = thread($b)

rule full-team-floor  over order  ratified D-0162
  : listener-has(configuration full-team) requires slot >= 10
";

const PROOF: &str = "\
scope magic

proof e01-s04  speaker protagonist  in spoken-proof {
  premise p1  holds(enfys, consent(household, door))  cited D-0113  cue \"line 112\"
  step   s1  instantiate ward-form  with ground = door, fibre = withy
  spend  p3
  close  holds(ward(door))  expect holds
  }
";

#[test]
fn expert_blocks_parse_clean() {
    for (label, source) in [
        ("practice", PRACTICE),
        ("time", TIME),
        ("language", LANGUAGE),
        ("craft", CRAFT),
        ("proof", PROOF),
    ] {
        let (_doc, diags) = parse_document(source);
        let errs: Vec<_> = diags.iter().filter(|d| d.class.is_error()).collect();
        assert!(errs.is_empty(), "{label} produced errors: {errs:?}");
    }
}

#[test]
fn practice_block_captures_its_body() {
    let (doc, diags) = parse_document(PRACTICE);
    let errs: Vec<_> = diags.iter().filter(|d| d.class.is_error()).collect();
    assert!(errs.is_empty(), "{errs:?}");
    let dli_ast::decl::Decl::Practice(practice) = &doc.decls[0] else {
        panic!("expected a practice");
    };
    assert_eq!(practice.name.text, "spoken-proof");
    // Every non-blank interior line is captured as a clause.
    assert_eq!(practice.body.len(), 7);
}
