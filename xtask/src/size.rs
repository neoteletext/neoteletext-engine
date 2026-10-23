//! The size and symbol checks of the linked microcontroller test image
//! (REQ-GEN-009, REQ-GEN-019, decision E.61): `cargo xtask size [--target
//! TRIPLE]`.
//!
//! It links `firmware/test-image` once for each build of [`BUILDS`], reads
//! the ELF image, prints its flash and static memory, and fails if a
//! budgeted build is over its budget or if any image contains a
//! floating-point routine, a compare-and-swap atomic routine, or a panic
//! path other than the image's own handler. It then links the image with a
//! deliberate floating-point operation and fails unless the symbol check
//! finds it, on targets without a floating-point unit, so the check is
//! known to work.

mod elf;

use elf::{Image, read_elf};
use std::fs;
use std::path::Path;
use std::process::Command;

/// The target REQ-GEN-009 and REQ-GEN-019 name: Cortex-M0, the smallest.
const DEFAULT_TARGET: &str = "thumbv6m-none-eabi";

/// REQ-GEN-009: the flash budget of the core, which "shall start at 64
/// kilobytes". It applies to the Level 1.5 build, the minimum
/// configuration REQ-GEN-009 describes; the other builds are reported.
const FLASH_BUDGET: u64 = 64 * 1024;

/// One build of the test image: its name, the image features it enables,
/// and its flash budget, if it has one.
struct Build {
    name: &'static str,
    features: &'static str,
    budget: Option<u64>,
}

/// The builds measured, smallest first (decision E.61).
const BUILDS: &[Build] = &[
    // Acquisition, the Level 1 grid and composing: the core with no
    // optional area.
    Build { name: "level1", features: "", budget: None },
    // A Level 1.5 receiver with the renderer and the viewer; the port
    // supplies its glyphs.
    Build { name: "level15", features: "level15 render viewer", budget: Some(FLASH_BUDGET) },
    // The same at Level 3.5, with every capability (decision E.71).
    Build { name: "level35", features: "level15 capabilities render viewer", budget: None },
    // The same with only the modern-classic profile's capabilities
    // (decision E.71, REQ-GEN-020).
    Build { name: "modern-classic", features: "level15 modern-classic render viewer", budget: None },
    // Every area of the core, with Bedstead.
    Build { name: "core-all", features: "level15 capabilities render bedstead viewer forms", budget: None },
    // Format detection reading T42 only, then every format; the core with
    // no optional area underneath.
    Build { name: "t42", features: "formats", budget: None },
    Build { name: "formats", features: "formats-all", budget: None },
    // The web user agent without push, forms and cookies, then with them.
    Build { name: "web", features: "web", budget: None },
    Build { name: "web-all", features: "web-all", budget: None },
];

/// Targets without a floating-point unit, where the float probe must be
/// found as a software routine.
const SOFT_FLOAT_TARGETS: &[&str] = &[
    "thumbv6m-none-eabi",
    "thumbv7m-none-eabi",
    "thumbv8m.base-none-eabi",
    "riscv32imc-unknown-none-elf",
    "riscv32imac-unknown-none-elf",
];

/// Parts of symbol names that mark a software floating-point routine.
const FLOAT_MARKERS: &[&str] = &[
    "compiler_builtins5float",
    "__aeabi_f",
    "__aeabi_d",
    "__aeabi_i2f",
    "__aeabi_ui2f",
    "__aeabi_l2f",
    "__aeabi_ul2f",
    "__aeabi_i2d",
    "__aeabi_ui2d",
    "__aeabi_l2d",
    "__aeabi_ul2d",
    "sf3",
    "df3",
    "sf2",
    "df2",
];

/// Parts of symbol names that mark a compare-and-swap atomic routine.
const ATOMIC_MARKERS: &[&str] = &["__sync_", "__atomic_", "compare_exchange", "cmpxchg"];

/// Parts of symbol names that mark a panic path in the `core` library: its
/// `panicking` module, and its out-of-line `..._fail` and `..._failed`
/// functions (bounds, slice lengths, `unwrap`).
const PANIC_MARKERS: &[&str] = &["4core9panicking", "_ZN4core9panicking", "panic_bounds_check"];

/// `cargo xtask size [--target TRIPLE]`.
pub fn run(root: &Path, args: &[String]) -> Result<(), String> {
    let target = match args {
        [] => DEFAULT_TARGET.to_string(),
        [flag, target] if flag == "--target" => target.clone(),
        _ => return Err("usage: cargo xtask size [--target TRIPLE]".to_string()),
    };
    let image_dir = root.join("firmware/test-image");
    let mut failures = Vec::new();
    println!("{:<14} {:>10} {:>10}  budget", "build", "flash", "static RAM");
    for build in BUILDS {
        let image = link(&image_dir, &target, build.features)?;
        let budget = build.budget.map_or_else(String::new, |budget| format!("{budget}"));
        println!("{:<14} {:>10} {:>10}  {budget}", build.name, image.flash(), image.ram());
        if let Some(budget) = build.budget
            && image.flash() > budget
        {
            failures.push(format!("{}: flash {} bytes is over the budget of {budget}", build.name, image.flash()));
        }
        for found in forbidden(image.symbols()) {
            failures.push(format!("{}: {found}", build.name));
        }
    }
    if SOFT_FLOAT_TARGETS.contains(&target.as_str()) {
        let probe = link(&image_dir, &target, "float-probe")?;
        if forbidden(probe.symbols()).iter().any(|found| found.starts_with("floating point")) {
            println!("float probe: found, as it must be");
        } else {
            failures.push("the deliberate floating-point operation was not found: the symbol check is broken".into());
        }
    } else {
        println!("float probe: skipped, {target} has a floating-point unit");
    }
    if failures.is_empty() {
        println!("ok");
        Ok(())
    } else {
        for failure in &failures {
            eprintln!("{failure}");
        }
        Err(format!("{} failure(s)", failures.len()))
    }
}

/// Links the test image with `features` and reads it.
fn link(image_dir: &Path, target: &str, features: &str) -> Result<Image, String> {
    let status = Command::new("cargo")
        .current_dir(image_dir)
        .args(["build", "--quiet", "--release", "--target", target, "--features", features])
        .status()
        .map_err(|e| format!("cargo build: {e}"))?;
    if !status.success() {
        return Err(format!("the test image does not build for {target} with features [{features}]"));
    }
    let path = image_dir.join("target").join(target).join("release/neoteletext-test-image");
    let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    read_elf(&bytes).ok_or_else(|| format!("{}: not a little-endian ELF image", path.display()))
}

/// The forbidden symbols of REQ-GEN-019, described.
fn forbidden(symbols: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    for symbol in symbols {
        if FLOAT_MARKERS.iter().any(|marker| is_float(symbol, marker)) {
            found.push(format!("floating point: {symbol}"));
        } else if ATOMIC_MARKERS.iter().any(|marker| symbol.contains(marker)) {
            found.push(format!("compare-and-swap atomic: {symbol}"));
        } else if is_panic(symbol) {
            found.push(format!("panic path: {symbol}"));
        }
    }
    found
}

/// The short markers (`sf3` and the like) are the suffixes of the C names
/// of the soft-float routines, such as `__mulsf3`; the others may appear
/// anywhere.
fn is_float(symbol: &str, marker: &str) -> bool {
    if marker.len() == 3 { symbol.starts_with("__") && symbol.ends_with(marker) } else { symbol.contains(marker) }
}

/// A panic path in `core`: its `panicking` module, or one of its
/// out-of-line failure functions such as `slice_index_fail`.
fn is_panic(symbol: &str) -> bool {
    PANIC_MARKERS.iter().any(|marker| symbol.contains(marker))
        || (symbol.contains("4core") && (symbol.contains("_fail") || symbol.contains("failed")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_are_classified() {
        let symbols: Vec<String> = [
            "__aeabi_fmul",
            "_RNvNtNtCs8_17compiler_builtins5float3mul8___mulsf3",
            "__sync_val_compare_and_swap_4",
            "_RNvNtCs1_4core9panicking5panic",
            "_RNvNtNtCs1_4core5slice5index16slice_index_fail",
            "_RNvMs3_NtCs3_12neoteletext_web5agent13fail_exchange",
            "_RNvCs1_19neoteletext_test_image5panic",
            "rust_begin_unwind",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
        let found = forbidden(&symbols);
        assert_eq!(found.len(), 5, "{found:?}");
        assert!(found.iter().all(|f| !f.contains("fail_exchange") && !f.contains("test_image")));
    }
}
