//! Door T (`AUTH-1`): the authoring workspace engine and the `author.*` tools.
//!
//! The frozen serve seam: `serve.rs` routes every `author.*` op through
//! [`dispatch`] — one call, one store. The workspace digest is the wire fence
//! (`expected_digest` / stale-fence in the envelope).

pub mod digest;
pub mod render;
pub mod store;
pub mod tools;
pub mod workspace;

use crate::envelope::{Request, Response};

/// The Door T dispatch seam (frozen verbatim in the wave-3 sockets): route an
/// `author.*` request against the workspace store and answer with graded
/// findings, rendered lines, and the new workspace digest.
#[allow(clippy::todo)]
pub fn dispatch(request: &Request, store: &mut store::Store) -> Response {
    let _ = (request, store);
    todo!("AUTH-1: dispatch author.* ops against the workspace store")
}
