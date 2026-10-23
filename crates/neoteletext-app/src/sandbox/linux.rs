//! The Linux sandbox: Landlock for files and the network, a seccomp filter
//! for processes and sockets (REQ-SEC-017, decision E.33 layer 5).

use super::Policy;
use landlock::{
    ABI, Access, AccessFs, AccessNet, PathBeneath, PathFd, Ruleset, RulesetAttr, RulesetCreatedAttr, RulesetStatus,
};
use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter, SeccompRule, TargetArch,
};
use std::collections::BTreeMap;

/// The newest Landlock ABI used: version 4 adds TCP restrictions.
const LANDLOCK_ABI: ABI = ABI::V4;

fn landlock(policy: Policy<'_>) -> Result<(), String> {
    let mut ruleset = Ruleset::default().handle_access(AccessFs::from_all(LANDLOCK_ABI)).map_err(|e| e.to_string())?;
    if !policy.network {
        ruleset = ruleset.handle_access(AccessNet::from_all(LANDLOCK_ABI)).map_err(|e| e.to_string())?;
    }
    let mut created = ruleset.create().map_err(|e| e.to_string())?;
    for directory in policy.readable {
        let fd = PathFd::new(directory).map_err(|e| e.to_string())?;
        created =
            created.add_rule(PathBeneath::new(fd, AccessFs::from_read(LANDLOCK_ABI))).map_err(|e| e.to_string())?;
    }
    if let Some(directory) = policy.writable_directory {
        let fd = PathFd::new(directory).map_err(|e| e.to_string())?;
        created =
            created.add_rule(PathBeneath::new(fd, AccessFs::from_all(LANDLOCK_ABI))).map_err(|e| e.to_string())?;
    }
    let status = created.restrict_self().map_err(|e| e.to_string())?;
    match status.ruleset {
        RulesetStatus::FullyEnforced | RulesetStatus::PartiallyEnforced => Ok(()),
        RulesetStatus::NotEnforced => Err("Landlock is not supported by this kernel".to_string()),
    }
}

/// The architectures seccompiler can write filters for; `None` on the
/// others, such as 32-bit Arm (IMPLEMENTATION-LOG.md decision 38).
fn target_arch() -> Option<TargetArch> {
    if cfg!(target_arch = "x86_64") {
        Some(TargetArch::x86_64)
    } else if cfg!(target_arch = "aarch64") {
        Some(TargetArch::aarch64)
    } else if cfg!(target_arch = "riscv64") {
        Some(TargetArch::riscv64)
    } else {
        None
    }
}

/// A system call number as seccompiler's rule key.
#[allow(clippy::useless_conversion, reason = "c_long is 64 bits here but 32 bits on 32-bit Arm")]
fn number(syscall: libc::c_long) -> i64 {
    i64::from(syscall)
}

fn seccomp(policy: Policy<'_>) -> Result<(), String> {
    let Some(arch) = target_arch() else {
        eprintln!(
            "neottx: note: no seccomp filter exists for this architecture; only the Landlock sandbox applies (REQ-SEC-017)"
        );
        return Ok(());
    };
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    // Starting programs and debugging other processes.
    for syscall in [libc::SYS_execve, libc::SYS_execveat, libc::SYS_ptrace, libc::SYS_process_vm_writev] {
        rules.insert(number(syscall), vec![]);
    }
    #[cfg(target_arch = "x86_64")]
    for syscall in [libc::SYS_fork, libc::SYS_vfork] {
        rules.insert(number(syscall), vec![]);
    }
    // clone() is allowed only for threads (CLONE_THREAD set).
    let process_clone = SeccompRule::new(vec![
        SeccompCondition::new(0, SeccompCmpArgLen::Qword, SeccompCmpOp::MaskedEq(libc::CLONE_THREAD as u64), 0)
            .map_err(|e| e.to_string())?,
    ])
    .map_err(|e| e.to_string())?;
    rules.insert(number(libc::SYS_clone), vec![process_clone]);
    if !policy.network {
        for syscall in [libc::SYS_socket, libc::SYS_connect, libc::SYS_bind, libc::SYS_listen, libc::SYS_accept4] {
            rules.insert(number(syscall), vec![]);
        }
    }
    let filter = SeccompFilter::new(rules, SeccompAction::Allow, SeccompAction::Errno(libc::EPERM as u32), arch)
        .map_err(|e| e.to_string())?;
    let program: BpfProgram = filter.try_into().map_err(|e: seccompiler::BackendError| e.to_string())?;
    seccompiler::apply_filter(&program).map_err(|e| e.to_string())?;
    // clone3 cannot be inspected; answering "not implemented" makes the
    // C library fall back to clone, which the rule above checks.
    let mut clone3 = BTreeMap::new();
    clone3.insert(number(libc::SYS_clone3), vec![]);
    let filter = SeccompFilter::new(clone3, SeccompAction::Allow, SeccompAction::Errno(libc::ENOSYS as u32), arch)
        .map_err(|e| e.to_string())?;
    let program: BpfProgram = filter.try_into().map_err(|e: seccompiler::BackendError| e.to_string())?;
    seccompiler::apply_filter(&program).map_err(|e| e.to_string())
}

pub(super) fn apply(policy: Policy<'_>) -> Result<(), String> {
    landlock(policy)
        .map_err(|e| format!("cannot apply the Landlock sandbox: {e} (use --no-sandbox to run without it)"))?;
    seccomp(policy).map_err(|e| format!("cannot apply the seccomp sandbox: {e} (use --no-sandbox to run without it)"))
}
