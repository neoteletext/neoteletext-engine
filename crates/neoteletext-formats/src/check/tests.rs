//! Page checker tests: one page per problem (REQ-CLI-006).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use super::*;
use crate::tti::tests::{FORM_EXAMPLE, read};
use std::vec::Vec;

fn findings(text: &[u8], options: &CheckOptions<'_>) -> Vec<(Severity, Problem, u32)> {
    let (pages, _) = read(text);
    let mut found = Vec::new();
    for page in &pages {
        check_page(page, options, &mut |finding| found.push((finding.severity, finding.problem, finding.line)));
    }
    found
}

fn check(text: &[u8]) -> Vec<(Severity, Problem, u32)> {
    findings(text, &CheckOptions::default())
}

const E3_EXAMPLE: &[u8] = b"PN,10000\nOL,1,NEWS INDEX\nOL,3,Headlines        ...\nLN,R,101,Headlines\nLN,G,140,Sport\n\
LN,Y,weather:400,Weather\nLN,C,190,TV guide\nLN,I,100,\n";

#[test]
fn appendix_e3_example_warns_as_the_appendix_says() {
    let warning = |warning, line| (Severity::Warning, Problem::Warning(warning), line);
    assert_eq!(
        check(E3_EXAMPLE),
        [
            warning(Warning::PageNumberNotShown, 4),
            warning(Warning::PageNumberNotShown, 5),
            warning(Warning::NoOtherRoute, 6),
            warning(Warning::PageNumberNotShown, 7),
        ]
    );
    let services: &[&[u8]] = &[b"news"];
    let options = CheckOptions { services: Some(services) };
    assert!(findings(E3_EXAMPLE, &options).contains(&(Severity::Error, Problem::UnknownService, 6)));
}

#[test]
fn shown_page_numbers_and_routes_silence_the_warnings() {
    let text = b"PN,10000\nOL,1,Headlines 101  Sport 140\nOL,2,Weather: www.example.com\nLN,R,101,Headlines\n\
LN,G,140,Sport\nLN,Y,weather:400,Weather\n";
    assert_eq!(check(text), []);
    // 1010 is not 101.
    assert_eq!(check(b"PN,10000\nOL,1,Call 1010\nLN,R,101,a\n").len(), 1);
}

#[test]
fn considered_warnings_are_not_reported() {
    let mut text = E3_EXAMPLE.to_vec();
    text.extend_from_slice(b"CK,considered,page-number-not-shown\nCK,considered,no-other-route\n");
    assert_eq!(check(&text), []);
}

#[test]
fn link_errors() {
    let found =
        check(b"PN,10000\nLN,R,101,a\nLN,G,102,b\nLN,Y,103,c\nLN,C,104,d\nLN,R,105,e\nOL,1,101 102 103 104 105\n");
    assert!(found.contains(&(Severity::Error, Problem::TooManyLinks, 6)));
    let found = check(b"PN,10000\nOL,1,101 102\nLN,R,101,Ten chars!\nLN,G,102,b\xA0d\nLN,Y,x:9ZZ,c\n");
    assert_eq!(
        found,
        [
            (Severity::Error, Problem::BadLinkTarget, 5),
            (Severity::Error, Problem::LabelTooLong, 3),
            (Severity::Error, Problem::LabelCharacters, 4),
        ]
    );
}

#[test]
fn hidden_row_24_content() {
    let found = check(b"PN,10000\nOL,1,101\nOL,24,Old prompts\nLN,R,101,a\n");
    assert_eq!(found, [(Severity::Warning, Problem::Warning(Warning::HiddenContent), 1)]);
}

#[test]
fn form_findings() {
    // The Appendix C.7 example has no fallback for broadcast viewers.
    assert_eq!(check(FORM_EXAMPLE), [(Severity::Warning, Problem::Warning(Warning::NoBroadcastFallback), 1)]);
    let mut with_phone = FORM_EXAMPLE.to_vec();
    with_phone.extend_from_slice(b"OL,20,Or call 0800 123 456\n");
    assert_eq!(check(&with_phone), []);
    // Placeholders on national option positions, a version 2.0 field, the
    // withdrawn FH, and overlapping fields.
    let text =
        b"PN,40001\nOL,22,www.example.com\nOL,3,Name: [______]\nFD,TEXT,name,6,8,,N,,\nFD,TEXT,old,50,Y\nFH,a,b\n\
FD,TEXT,clash,7,3,,N,,\n";
    let found = check(text);
    assert!(found.contains(&(Severity::Warning, Problem::Warning(Warning::UnreliablePlaceholders), 4)), "{found:?}");
    assert!(found.contains(&(Severity::Error, Problem::FormVersion2, 5)));
    assert!(found.contains(&(Severity::Error, Problem::WithdrawnCommand, 6)));
    assert!(found.contains(&(Severity::Error, Problem::BadFormField, 7)));
}

#[test]
fn warning_names_round_trip() {
    for warning in Warning::ALL {
        assert_eq!(Warning::from_name(warning.name()), Some(warning));
    }
    assert_eq!(Warning::from_name(b"HIDDEN-CONTENT"), Some(Warning::HiddenContent));
}

/// the web-native concept section 5: a page that links a number it
/// does not show is warned about; one that shows it is not.
#[test]
fn linked_numbers_must_be_shown() {
    let page = b"PN,10000\nOL,4,Weather 300\nLK,300,https://w.example/300.t42\nLK,400,https://s.example/\n";
    assert_eq!(check(page), [(Severity::Warning, Problem::Warning(Warning::PageNumberNotShown), 4)]);
}

/// REQ-CLI-006, decision E.111: a `TF` line is a withdrawn command, an
/// error, however it is written.
#[test]
fn font_lines_are_withdrawn_commands() {
    let sha = "5ee6246b7f9bd197f47ce0741ea799a55ac08af775869a08c085803e02a3f14d";
    for page in [std::format!("PN,10000\nTF,f.bdf,{sha}\nOL,1,A\n"), "PN,10000\nTF,f.bdf,123\nOL,1,A\n".into()] {
        assert!(check(page.as_bytes()).contains(&(Severity::Error, Problem::WithdrawnCommand, 2)), "{page}");
    }
}
