//! The platforms without a sandbox yet (REQ-SEC-017 Phase 2): `neottx`
//! says so and continues (decision 15 of IMPLEMENTATION-LOG.md).

use super::Policy;

// The same signature as the Linux implementation, which can fail.
#[allow(clippy::unnecessary_wraps)]
pub(super) fn apply(policy: Policy<'_>) -> Result<(), String> {
    let files =
        policy.writable_directory.map_or_else(|| "files".to_string(), |dir| format!("files outside {}", dir.display()));
    let network = if policy.network { "" } else { " and the network" };
    let _ = policy.readable;
    eprintln!(
        "neottx: note: self-sandboxing is not implemented on this system yet (REQ-SEC-017 Phase 2); access to {files}{network} is not blocked"
    );
    Ok(())
}
