//! The fixed list of feature builds the gate lints and tests (decision
//! E.61, rule 6): `cargo xtask features`.
//!
//! Every crate's default build is covered by the workspace run with all
//! features. This list adds each crate with no optional area and with each
//! area alone, because a `cfg` mistake shows only in the builds that leave
//! something out. It is a fixed list rather than every combination: the
//! features are additive, so each area alone and none at all cover the
//! seams.

use std::path::Path;
use std::process::Command;

/// One build: the package and its features ("" for none).
struct Build {
    package: &'static str,
    features: &'static str,
}

/// The builds, crate by crate, with no default features.
const BUILDS: &[Build] = &[
    Build { package: "neoteletext-core", features: "" },
    Build { package: "neoteletext-core", features: "level15" },
    Build { package: "neoteletext-core", features: "level25" },
    // Levels 2.5 and 3.5 with every capability compiled out, and the
    // modern-classic profile (decision E.71, REQ-GEN-020).
    Build { package: "neoteletext-core", features: "level15,level25" },
    Build { package: "neoteletext-core", features: "modern-classic" },
    Build { package: "neoteletext-core", features: "render" },
    Build { package: "neoteletext-core", features: "bedstead" },
    Build { package: "neoteletext-core", features: "original" },
    Build { package: "neoteletext-core", features: "viewer" },
    Build { package: "neoteletext-core", features: "forms" },
    Build { package: "neoteletext-formats", features: "" },
    Build { package: "neoteletext-formats", features: "transport" },
    Build { package: "neoteletext-formats", features: "sliced" },
    Build { package: "neoteletext-formats", features: "pages" },
    Build { package: "neoteletext-formats", features: "tti" },
    Build { package: "neoteletext-formats", features: "ep1,edittf,dump" },
    Build { package: "neoteletext-formats", features: "check" },
    Build { package: "neoteletext-formats", features: "std" },
    Build { package: "neoteletext-formats", features: "std,tti" },
    Build { package: "neoteletext-web", features: "" },
    Build { package: "neoteletext-web", features: "push" },
    Build { package: "neoteletext-web", features: "forms" },
    Build { package: "neoteletext-web", features: "cookies" },
    Build { package: "neoteletext-app", features: "" },
    Build { package: "neoteletext-app", features: "live" },
    Build { package: "neoteletext-app", features: "slice" },
    Build { package: "neoteletext-cli", features: "" },
    Build { package: "neoteletext-cli", features: "view" },
    Build { package: "neoteletext-cli", features: "live" },
    Build { package: "neoteletext-cli", features: "slice" },
];

/// `cargo xtask features`: lints and tests each build of [`BUILDS`].
pub fn run(root: &Path) -> Result<(), String> {
    let mut failures = Vec::new();
    for build in BUILDS {
        let label = format!("{} [{}]", build.package, build.features);
        let base = ["--quiet", "-p", build.package, "--no-default-features", "--features", build.features];
        let lint = cargo(root, &[&["clippy", "--all-targets"][..], &base, &["--", "-D", "warnings"]].concat());
        let test = lint && cargo(root, &[&["test"][..], &base].concat());
        println!("{label:<40} {}", if test { "ok" } else { "FAILED" });
        if !test {
            failures.push(label);
        }
    }
    if failures.is_empty() { Ok(()) } else { Err(format!("failed: {}", failures.join(", "))) }
}

/// Runs cargo with `args`, its output shown; true on success.
fn cargo(root: &Path, args: &[&str]) -> bool {
    Command::new("cargo").current_dir(root).args(args).status().is_ok_and(|status| status.success())
}
