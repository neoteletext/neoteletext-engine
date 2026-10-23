//! Running one command: the arguments parsed, the command's function
//! called, and its failure turned into the exit status (REQ-CLI-005).

use crate::args::{Cli, Command, FontCommand};
use crate::commands;
use clap::Parser;
use neoteletext_app::{exit, sandbox};
use std::process::ExitCode;

/// Runs `neottx` with the process's arguments, returning its exit status.
#[must_use]
pub fn run() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            return if error.use_stderr() { ExitCode::from(exit::USAGE) } else { ExitCode::SUCCESS };
        }
    };
    let result = match cli.command {
        Command::List { common } => commands::list(&common.options()),
        Command::Show { common, page, raster, graphics, legacy_mosaics, colours8 } => {
            commands::show(&common.options(), &page, &raster, graphics, legacy_mosaics, colours8)
        }
        Command::View { common, raster, graphics, legacy_mosaics, colours8 } => {
            commands::view(&common.options(), &raster, graphics, legacy_mosaics, colours8)
        }
        Command::Render { common, page, output, raster, image } => {
            commands::render(&common.options(), &page, &output, &raster, image)
        }
        Command::Text { common, page, reveal, legacy_mosaics, cells } => {
            commands::text(&common.options(), &page, reveal, legacy_mosaics, cells)
        }
        Command::Convert { common, output, to, page } => {
            commands::convert(&common.options(), &output, to, page.as_deref())
        }
        Command::Slice { common, output, preset } => commands::slice(&common.options(), &output, &preset),
        Command::Services { common } => commands::services(&common.options()),
        Command::ServiceInfo { common } => commands::service_info(&common.options()),
        Command::Check { files, descriptor } => commands::check(&files, descriptor.as_deref()),
        Command::Font { action: FontCommand::Convert { input, output, map, no_sandbox } } => {
            commands::convert_font(&input, &output, map.as_deref(), no_sandbox)
        }
        Command::Font { action: FontCommand::Sets { font, library_line, no_sandbox } } => {
            commands::font_sets(&font, library_line.as_deref(), no_sandbox)
        }
        Command::Contents { common } => commands::contents(&common.options()),
        Command::WriteText { set, page, input, output, no_sandbox } => {
            commands::write_text(&set, &page, &input, &output, no_sandbox)
        }
        Command::Sound {
            page,
            channel,
            gain,
            codec,
            handle,
            looped,
            repeat,
            live,
            pace,
            input,
            output,
            no_sandbox,
        } => {
            let options = commands::SoundOptions { page, channel, gain, codec, handle, looped, repeat, live, pace };
            commands::sound(&options, &input, &output, no_sandbox)
        }
        #[cfg(feature = "keyboard")]
        Command::Qr { text, page, row, column, output } => commands::qr(&text, &page, row, column, &output),
        #[cfg(feature = "keyboard")]
        Command::Keyboard { channel, relay, pin, name, qr_page, qr_output, proxy } => {
            let qr_page = qr_page.zip(qr_output);
            commands::keyboard(&commands::KeyboardOptions { channel, relay, pin, name, qr_page, proxy })
        }
        #[cfg(not(feature = "keyboard"))]
        Command::Qr { .. } | Command::Keyboard { .. } => {
            Err(neoteletext_app::not_in_build("qr and keyboard", "keyboard"))
        }
        Command::SandboxTest => sandbox::self_test(),
        Command::Fetch { address, no_sandbox, allow_local, proxy } => {
            commands::fetch(&address, no_sandbox, &allow_local, proxy.as_deref())
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err((code, message)) => {
            eprintln!("neottx: {message}");
            ExitCode::from(code)
        }
    }
}
