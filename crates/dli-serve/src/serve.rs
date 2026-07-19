//! The serve loop and request dispatch (SERVE-1).

use crate::envelope::{Request, Response};

/// Route a single request to its handler and build the response.
pub fn dispatch(_request: &Request) -> Response {
    todo!("SERVE-1: route each Op to its handler and build the response envelope")
}

/// Run the newline-delimited JSON serve loop over stdio.
pub fn serve() -> std::io::Result<()> {
    todo!("SERVE-1: read NDJSON requests from stdin, dispatch, write responses")
}
