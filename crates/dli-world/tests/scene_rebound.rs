//! End-to-end rebound: a proof premise the certain layer *contradicts* must fail
//! at the practice's L2-*contradicted* error (`forsworn`), not its L2-*absent* one
//! (`unowed`). This exercises the whole seam — `project_scene` reads the world's
//! option-exclusivity into [`SceneProjection::denied`], and `check_proof` reads
//! that denial as a rebound (`docs/spec/02-semantics.md` §4, `04-practices.md` §2).

use dli_ledger::ledger_check::{ProofFinding, ProofVerdict, check_proof};
use dli_ledger::practice::{StructuralClass, compile_practices};
use dli_syntax::parse_document;

use dli_world::pack::Pack;
use dli_world::scene_projection::project_scene;
use dli_world::{CompiledWorld, compile};

/// Arthur has *refused*; consent and refusal are the two exclusive answers to one
/// question, and a swearer's oath rests on Arthur's consent. The certain layer
/// refutes that consent, so the casting REBOUNDS at `forsworn` (L2 contradicted),
/// the class only a populated `denied` can reach — a fizzle at `unowed` (L2 absent)
/// would be the wrong verdict.
const WORLD: &str = "\
scope demo
ratify d-1
refused(arthur)   decided d-1
question arthurs-answer \"Does Arthur consent to the oath?\"
  option refuses  implies refused(arthur)
  option grants   implies consent(arthur)
practice oath-binding  proposed {
  role oathholder : person   requires consent(oathholder)
  resource boon : linear conserved
  rule pledge(bond, oathgiver)
  error unowed   refines L2 premise-not-available where absent
                 # a pledge drawn on a boon never granted
  error forsworn refines L2 premise-not-available where contradicted signals dishonour
                 # a boon the ledger contradicts recoils on the swearer
  cost social
}
proof arthurs-oath  speaker lancelot  in oath-binding {
  premise p1  consent(arthur)  cited d-1  cue \"Lancelot swears on the king's consent\"
  close pledged(lancelot, fealty) expect fails-at forsworn
}
";

fn compiled() -> (CompiledWorld, dli_ast::decl::Document) {
    let (doc, diags) = parse_document(WORLD);
    assert!(diags.is_empty(), "the world parses: {diags:?}");
    let world = compile(&Pack {
        name: "demo".to_owned(),
        documents: vec![doc.clone()],
    })
    .expect("the world compiles");
    (world, doc)
}

#[test]
fn a_contradicted_premise_rebounds_at_forsworn() {
    let (world, doc) = compiled();
    let practice = compile_practices(&doc)
        .into_iter()
        .find(|p| p.name.text == "oath-binding")
        .expect("the oath-binding practice compiles");
    let proof = doc
        .decls
        .iter()
        .find_map(|d| match d {
            dli_ast::decl::Decl::Proof(p) => Some(p),
            _ => None,
        })
        .expect("the proof is present");

    // The projection reads the world's exclusivity: consent(arthur) is denied,
    // because the certain refused(arthur) is its exclusive sibling.
    let scene = project_scene(&world, Some("lancelot"));
    assert!(
        scene.denied.contains("consent(arthur)"),
        "the certain layer denies the excluded sibling: {:?}",
        scene.denied,
    );

    let report = check_proof(&practice, proof, &scene);
    assert_eq!(
        report.verdict,
        ProofVerdict::FailedAt {
            class: StructuralClass::L2,
            error: Some("forsworn".to_owned()),
            at: "p1".to_owned(),
        },
        "a contradicted premise rebounds at the L2-contradicted error, not the absent one",
    );
    let messages: Vec<String> = report.findings.iter().map(ProofFinding::message).collect();
    assert!(
        report.checks,
        "the proof declared `fails-at forsworn` and met it: {messages:?}",
    );
}
