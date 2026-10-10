//! Self-sandboxing (REQ-SEC-017, decision E.33 layer 5).
//!
//! After opening its inputs and outputs, `ttx` restricts itself so that it
//! cannot open other files, start programs, or use the network outside web
//! mode. Linux uses Landlock and a seccomp filter (Phase 1). Other systems
//! are Phase 2: `ttx` says so and continues (decision 15 of
//! IMPLEMENTATION-LOG.md).

use std::path::Path;

/// What the sandboxed process may still do.
#[derive(Clone, Copy, Debug)]
pub struct Policy<'a> {
    /// A directory the process may create and write files in.
    pub writable_directory: Option<&'a Path>,
    /// The process may make network connections (web mode fetcher).
    pub network: bool,
}

/// Applies the sandbox. `allow_unsandboxed` is `--no-sandbox`.
pub fn apply(policy: Policy<'_>, allow_unsandboxed: bool) -> Result<(), String> {
    if allow_unsandboxed {
        eprintln!("ttx: running without a sandbox (--no-sandbox)");
        return Ok(());
    }
    platform::apply(policy)
}

#[cfg(target_os = "linux")]
mod platform {
    use super::Policy;
    use landlock::{
        ABI, Access, AccessFs, AccessNet, PathBeneath, PathFd, Ruleset, RulesetAttr, RulesetCreatedAttr, RulesetStatus,
    };
    use seccompiler::{
        BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter, SeccompRule,
        TargetArch,
    };
    use std::collections::BTreeMap;

    /// The newest Landlock ABI used: version 4 adds TCP restrictions.
    const LANDLOCK_ABI: ABI = ABI::V4;

    fn landlock(policy: Policy<'_>) -> Result<(), String> {
        let mut ruleset = Ruleset::default()
            .handle_access(AccessFs::from_all(LANDLOCK_ABI))
            .map_err(|e| e.to_string())?;
        if !policy.network {
            ruleset = ruleset.handle_access(AccessNet::from_all(LANDLOCK_ABI)).map_err(|e| e.to_string())?;
        }
        let mut created = ruleset.create().map_err(|e| e.to_string())?;
        if let Some(directory) = policy.writable_directory {
            let fd = PathFd::new(directory).map_err(|e| e.to_string())?;
            created = created
                .add_rule(PathBeneath::new(fd, AccessFs::from_all(LANDLOCK_ABI)))
                .map_err(|e| e.to_string())?;
        }
        let status = created.restrict_self().map_err(|e| e.to_string())?;
        match status.ruleset {
            RulesetStatus::FullyEnforced | RulesetStatus::PartiallyEnforced => Ok(()),
            RulesetStatus::NotEnforced => Err("Landlock is not supported by this kernel".to_string()),
        }
    }

    fn target_arch() -> Result<TargetArch, String> {
        if cfg!(target_arch = "x86_64") {
            Ok(TargetArch::x86_64)
        } else if cfg!(target_arch = "aarch64") {
            Ok(TargetArch::aarch64)
        } else {
            Err("no seccomp filter for this architecture".to_string())
        }
    }

    fn seccomp(policy: Policy<'_>) -> Result<(), String> {
        let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
        // Starting programs and debugging other processes.
        for syscall in [libc::SYS_execve, libc::SYS_execveat, libc::SYS_ptrace, libc::SYS_process_vm_writev] {
            rules.insert(syscall, vec![]);
        }
        #[cfg(target_arch = "x86_64")]
        for syscall in [libc::SYS_fork, libc::SYS_vfork] {
            rules.insert(syscall, vec![]);
        }
        // clone() is allowed only for threads (CLONE_THREAD set).
        let process_clone = SeccompRule::new(vec![
            SeccompCondition::new(
                0,
                SeccompCmpArgLen::Qword,
                SeccompCmpOp::MaskedEq(libc::CLONE_THREAD as u64),
                0,
            )
            .map_err(|e| e.to_string())?,
        ])
        .map_err(|e| e.to_string())?;
        rules.insert(libc::SYS_clone, vec![process_clone]);
        if !policy.network {
            for syscall in [libc::SYS_socket, libc::SYS_connect, libc::SYS_bind, libc::SYS_listen, libc::SYS_accept4] {
                rules.insert(syscall, vec![]);
            }
        }
        let filter = SeccompFilter::new(
            rules,
            SeccompAction::Allow,
            SeccompAction::Errno(libc::EPERM as u32),
            target_arch()?,
        )
        .map_err(|e| e.to_string())?;
        let program: BpfProgram = filter.try_into().map_err(|e: seccompiler::BackendError| e.to_string())?;
        seccompiler::apply_filter(&program).map_err(|e| e.to_string())?;
        // clone3 cannot be inspected; answering "not implemented" makes the
        // C library fall back to clone, which the rule above checks.
        let mut clone3 = BTreeMap::new();
        clone3.insert(libc::SYS_clone3, vec![]);
        let filter = SeccompFilter::new(
            clone3,
            SeccompAction::Allow,
            SeccompAction::Errno(libc::ENOSYS as u32),
            target_arch()?,
        )
        .map_err(|e| e.to_string())?;
        let program: BpfProgram = filter.try_into().map_err(|e: seccompiler::BackendError| e.to_string())?;
        seccompiler::apply_filter(&program).map_err(|e| e.to_string())
    }

    pub(super) fn apply(policy: Policy<'_>) -> Result<(), String> {
        landlock(policy).map_err(|e| format!("cannot apply the Landlock sandbox: {e} (use --no-sandbox to run without it)"))?;
        seccomp(policy).map_err(|e| format!("cannot apply the seccomp sandbox: {e} (use --no-sandbox to run without it)"))
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::Policy;

    pub(super) fn apply(_policy: Policy<'_>) -> Result<(), String> {
        eprintln!("ttx: note: self-sandboxing is not implemented on this system yet (REQ-SEC-017 Phase 2)");
        Ok(())
    }
}
