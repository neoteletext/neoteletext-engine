use super::*;
use neoteletext_app::exit;

fn parse(arguments: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(["neottx"].iter().chain(arguments))
}

fn level(arguments: &[&str]) -> Option<LevelChoice> {
    match parse(arguments).map(|cli| cli.command) {
        Ok(Command::List { common }) => common.level,
        other => panic!("not a list command: {other:?}"),
    }
}

/// REQ-CLI-007: each level is accepted, any other value is invalid
/// usage, and the former `--level1-only` option is gone.
#[test]
fn level_option_takes_the_four_levels() {
    assert_eq!(level(&["list", "in.t42"]), None);
    for (text, expected) in [
        ("1", LevelChoice::Level1),
        ("1.5", LevelChoice::Level15),
        ("2.5", LevelChoice::Level25),
        ("3.5", LevelChoice::Level35),
    ] {
        assert_eq!(level(&["list", "in.t42", "--level", text]), Some(expected), "{text}");
    }
    for invalid in ["2", "1.0", "4", "level1"] {
        let error = parse(&["list", "in.t42", "--level", invalid]).unwrap_err();
        assert!(error.use_stderr(), "{invalid} is invalid usage (exit status {})", exit::USAGE);
    }
    assert!(parse(&["list", "in.t42", "--level1-only"]).is_err());
}
