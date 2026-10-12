//! `neoteletext`: the browser window, or one of its helpers.
//!
//! - `neoteletext [--allow-local ADDRESS]... [--data DIRECTORY] [ADDRESS]...`
//!   opens the window, with a tab for each address.
//! - `neoteletext fetch ADDRESS` and `neoteletext decode` are the helpers
//!   the window starts for each tab; they are not for running by hand.

use eframe::egui;
use neoteletext_gui::app::BrowserApp;
use neoteletext_gui::helpers::{ALLOW_LOCAL, DECODE_COMMAND, FETCH_COMMAND, NO_SANDBOX, Options, SANDBOXED_BY_PARENT};
use neoteletext_gui::storage::Storage;
use std::net::IpAddr;
use std::path::PathBuf;
use std::process::ExitCode;

/// The hidden command that reports what the sandbox it runs in allows:
/// the end-to-end tests run it under each helper's profile.
const SANDBOX_TEST_COMMAND: &str = "sandbox-test";

/// Tries to read a file, connect to `address`, and start a program, and
/// prints which of them worked.
fn sandbox_test(address: Option<&str>) {
    let file = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".zshrc"))
        .is_some_and(|path| std::fs::read(path).is_ok() || std::fs::read("/etc/passwd").is_ok());
    let network = address.and_then(|address| address.parse::<std::net::SocketAddr>().ok()).is_some_and(|address| {
        std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_secs(2)).is_ok()
    });
    let program = std::process::Command::new("/usr/bin/true").status().is_ok();
    println!("file={file} network={network} program={program}");
}

/// Chooses another directory for the saved files.
const DATA: &str = "--data";
/// The window's first size, in points: a page at twice its size with the
/// toolbars.
const WINDOW_SIZE: [f32; 2] = [1000.0, 1130.0];
/// The smallest window: a page at its own size with the toolbars.
const WINDOW_MINIMUM: [f32; 2] = [520.0, 640.0];

struct Arguments {
    options: Options,
    no_sandbox: bool,
    sandboxed_by_parent: bool,
    data: Option<PathBuf>,
    rest: Vec<String>,
}

fn arguments() -> Result<Arguments, String> {
    let program = std::env::current_exe().map_err(|error| format!("cannot find this program: {error}"))?;
    let mut parsed = Arguments {
        options: Options { program, allow_local: Vec::new(), no_sandbox: false },
        no_sandbox: false,
        sandboxed_by_parent: false,
        data: None,
        rest: Vec::new(),
    };
    let mut items = std::env::args().skip(1);
    while let Some(item) = items.next() {
        match item.as_str() {
            NO_SANDBOX => parsed.no_sandbox = true,
            SANDBOXED_BY_PARENT => parsed.sandboxed_by_parent = true,
            ALLOW_LOCAL => {
                let address = items.next().ok_or("--allow-local needs an address")?;
                let address: IpAddr =
                    address.parse().map_err(|_| format!("--allow-local: {address}: not an address"))?;
                parsed.options.allow_local.push(address);
            }
            DATA => parsed.data = Some(PathBuf::from(items.next().ok_or("--data needs a directory")?)),
            _ => parsed.rest.push(item),
        }
    }
    parsed.options.no_sandbox = parsed.no_sandbox;
    Ok(parsed)
}

fn window(arguments: Arguments) -> Result<(), String> {
    let fonts = neoteletext_gui::system_fonts()?;
    let storage = Storage::new(arguments.data);
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("neoteletext")
            .with_inner_size(WINDOW_SIZE)
            .with_min_inner_size(WINDOW_MINIMUM)
            .with_drag_and_drop(true),
        ..eframe::NativeOptions::default()
    };
    let options = arguments.options;
    let addresses = arguments.rest;
    eframe::run_native(
        "neoteletext",
        native,
        Box::new(move |creation| {
            creation.egui_ctx.set_fonts(fonts);
            creation.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(BrowserApp::new(&creation.egui_ctx, options, storage, &addresses)))
        }),
    )
    .map_err(|error| error.to_string())
}

fn main() -> ExitCode {
    let arguments = match arguments() {
        Ok(arguments) => arguments,
        Err(message) => {
            eprintln!("neoteletext: {message}");
            return ExitCode::from(neoteletext_app::exit::USAGE);
        }
    };
    let result = match arguments.rest.first().map(String::as_str) {
        Some(FETCH_COMMAND) => match arguments.rest.get(1) {
            Some(address) => neoteletext_app::fetch::run(
                address,
                arguments.no_sandbox,
                arguments.sandboxed_by_parent,
                &arguments.options.allow_local,
            ),
            None => Err((neoteletext_app::exit::USAGE, "fetch needs an address".to_string())),
        },
        Some(SANDBOX_TEST_COMMAND) => {
            sandbox_test(arguments.rest.get(1).map(String::as_str));
            Ok(())
        }
        Some(DECODE_COMMAND) => neoteletext_app::decoder::run(
            arguments.no_sandbox,
            arguments.sandboxed_by_parent,
            &neoteletext_gui::BEDSTEAD,
        ),
        _ => window(arguments).map_err(|message| (neoteletext_app::exit::INPUT, message)),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err((status, message)) => {
            eprintln!("neoteletext: {message}");
            ExitCode::from(status)
        }
    }
}
