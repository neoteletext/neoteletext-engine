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
    /// Directories the process may read (the fetcher reads `/etc` to
    /// resolve names).
    pub readable: &'a [&'a Path],
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
        let mut ruleset =
            Ruleset::default().handle_access(AccessFs::from_all(LANDLOCK_ABI)).map_err(|e| e.to_string())?;
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
                "ttx: note: no seccomp filter exists for this architecture; only the Landlock sandbox applies (REQ-SEC-017)"
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
        seccomp(policy)
            .map_err(|e| format!("cannot apply the seccomp sandbox: {e} (use --no-sandbox to run without it)"))
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::Policy;

    // The same signature as the Linux implementation, which can fail.
    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn apply(policy: Policy<'_>) -> Result<(), String> {
        let files = policy
            .writable_directory
            .map_or_else(|| "files".to_string(), |dir| format!("files outside {}", dir.display()));
        let network = if policy.network { "" } else { " and the network" };
        let _ = policy.readable;
        eprintln!(
            "ttx: note: self-sandboxing is not implemented on this system yet (REQ-SEC-017 Phase 2); access to {files}{network} is not blocked"
        );
        Ok(())
    }
}

/// A helper process of the browser (REQ-SEC-026, `docs/desktop-browser.md`
/// section 3.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Helper {
    /// The fetcher: the network only.
    Fetcher,
    /// A decoder: nothing at all.
    Decoder,
}

/// A command that starts `program` as a helper. On macOS the helper is
/// confined from its first instruction by a Seatbelt profile applied by
/// `sandbox-exec` (decision recorded in `docs/macos-build-decisions.md`),
/// and the second value is true: the helper is told not to apply its
/// own. Elsewhere the helper applies its own sandbox (Landlock and
/// seccomp on Linux) and the second value is false.
pub fn helper_command(program: &Path, helper: Helper) -> Result<(std::process::Command, bool), String> {
    platform_helper(program, helper)
}

#[cfg(target_os = "macos")]
fn platform_helper(program: &Path, helper: Helper) -> Result<(std::process::Command, bool), String> {
    let path = program.to_str().ok_or("the program's path is not text")?;
    // The path is placed inside a quoted profile string.
    if path.contains(['"', '\\', '\n']) {
        return Err(format!("{path}: a character the sandbox profile cannot quote"));
    }
    let profile = match helper {
        Helper::Decoder => macos::DECODER,
        Helper::Fetcher => macos::FETCHER,
    }
    .replace(macos::PROGRAM, path);
    let mut command = std::process::Command::new(macos::SANDBOX_EXEC);
    command.arg("-p").arg(profile).arg(program);
    Ok((command, true))
}

#[cfg(not(target_os = "macos"))]
// The same signature as the macOS implementation, which can fail.
#[allow(clippy::unnecessary_wraps)]
fn platform_helper(program: &Path, _helper: Helper) -> Result<(std::process::Command, bool), String> {
    Ok((std::process::Command::new(program), false))
}

/// The Seatbelt profiles of the browser's helpers on macOS. Each denies
/// everything, then allows only what starting the program needs: running
/// itself, reading the system libraries and its own file, and the root
/// directory, which the loader reads at start. Measured on macOS 15.6.
#[cfg(target_os = "macos")]
mod macos {
    /// The program that applies a profile before starting a program.
    pub const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
    /// Replaced by the helper's own path.
    pub const PROGRAM: &str = "@PROGRAM@";
    /// A decoder: no network, no files, no other programs.
    pub const DECODER: &str = r#"(version 1)
(deny default)
(allow process-exec (literal "@PROGRAM@"))
(allow file-read* (literal "/") (literal "@PROGRAM@") (subpath "/usr/lib") (subpath "/System")
    (subpath "/private/var/db/dyld") (literal "/dev/urandom") (literal "/dev/null"))
(allow sysctl-read)
"#;
    /// The fetcher: a decoder's allowances, outgoing connections, and
    /// name resolution through the system's resolver service.
    pub const FETCHER: &str = r#"(version 1)
(deny default)
(allow process-exec (literal "@PROGRAM@"))
(allow file-read* (literal "/") (literal "@PROGRAM@") (subpath "/usr/lib") (subpath "/System")
    (subpath "/private/var/db/dyld") (literal "/dev/urandom") (literal "/dev/null")
    (literal "/etc") (literal "/private/etc/hosts") (literal "/private/etc/resolv.conf")
    (literal "/private/var/run/resolv.conf") (literal "/private/etc/services") (literal "/private/etc/protocols"))
(allow file-read-metadata)
(allow sysctl-read)
(allow system-socket)
(allow network-outbound)
(allow mach-lookup (global-name "com.apple.dnssd.service") (global-name "com.apple.SystemConfiguration.configd")
    (global-name "com.apple.SystemConfiguration.DNSConfiguration"))
"#;
}

/// The REQ-SEC-017 verification: after the sandbox is applied, opening a
/// file, starting a program and connecting to the network must all fail.
pub fn self_test() -> Result<(), (u8, String)> {
    apply(Policy { writable_directory: None, network: false, readable: &[] }, false)
        .map_err(|message| (crate::exit::INPUT, message))?;
    let file = std::fs::File::open("/etc/hostname").is_err();
    let program = std::process::Command::new("/bin/true").status().is_err();
    let network = std::net::TcpStream::connect(("1.1.1.1", 80)).is_err();
    println!("open file blocked: {file}\nstart program blocked: {program}\nnetwork blocked: {network}");
    if file && program && network {
        Ok(())
    } else {
        Err((crate::exit::INPUT, "the sandbox did not block everything".to_string()))
    }
}
