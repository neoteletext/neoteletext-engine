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
    Build { package: "tellytxt-core", features: "" },
    Build { package: "tellytxt-core", features: "level15" },
    Build { package: "tellytxt-core", features: "level25" },
    Build { package: "tellytxt-core", features: "render" },
    Build { package: "tellytxt-core", features: "bedstead" },
    Build { package: "tellytxt-core", features: "viewer" },
    Build { package: "tellytxt-core", features: "forms" },
    Build { package: "tellytxt-formats", features: "" },
    Build { package: "tellytxt-formats", features: "transport" },
    Build { package: "tellytxt-formats", features: "sliced" },
    Build { package: "tellytxt-formats", features: "pages" },
    Build { package: "tellytxt-formats", features: "tti" },
    Build { package: "tellytxt-formats", features: "ep1,edittf,dump" },
    Build { package: "tellytxt-formats", features: "check" },
    Build { package: "tellytxt-formats", features: "std" },
    Build { package: "tellytxt-formats", features: "std,tti" },
    Build { package: "tellytxt-web", features: "" },
    Build { package: "tellytxt-web", features: "push" },
    Build { package: "tellytxt-web", features: "forms" },
    Build { package: "tellytxt-web", features: "cookies" },
    Build { package: "tellytxt-app", features: "" },
    Build { package: "tellytxt-app", features: "live" },
    Build { package: "tellytxt-app", features: "slice" },
    Build { package: "tellytxt-cli", features: "" },
    Build { package: "tellytxt-cli", features: "view" },
    Build { package: "tellytxt-cli", features: "live" },
    Build { package: "tellytxt-cli", features: "slice" },
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
