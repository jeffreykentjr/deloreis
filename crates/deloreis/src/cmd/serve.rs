//! `deloreis serve` — delegate to the JSON-over-stdio worker loop.
//!
//! The worker itself lives in `dli-serve` (the SERVE-1 milestone, owned by the
//! server lane): read one JSON request per stdin line, dispatch, write one
//! response per line, loop to EOF. This subcommand is the thin delegation — it
//! forwards to the frozen entry point `dli_serve::serve::serve()` and maps its
//! `io::Result` to an exit code (clean EOF → `0`, an I/O failure → usage).
//!
//! The loop never panics on `dli.*` traffic — ops whose engine has not landed
//! answer with a typed capability error, not a crash. The one caller-reachable
//! panic is `author.*` traffic, which the worker routes through Door T
//! (`author::dispatch`, unimplemented until AUTH-1); that is a property of the
//! worker, not of this delegation.

use crate::cmd::{CmdResult, EXIT_CLEAN};

/// Run the JSON-over-stdio worker loop.
pub(crate) fn run() -> CmdResult {
    dli_serve::serve::serve()
        .map(|()| EXIT_CLEAN)
        .map_err(|err| format!("the worker loop failed: {err}"))
}
