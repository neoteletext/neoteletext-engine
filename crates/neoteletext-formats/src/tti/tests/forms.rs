//! The Form Extension (Appendix C.7): fields, options, submission and the
//! problems the reader records.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::cast_possible_truncation
)]

use super::*;

/// REQ-WEB-010: field definitions with columns, field types, options, the
/// token, the submission target and the result pages.
#[test]
fn form_example_of_appendix_c7() {
    let (pages, diagnostics) = read(FORM_EXAMPLE);
    assert_eq!(diagnostics, []);
    let form = &pages[0].form;
    assert!(form.interactive);
    let fields = form.fields.as_slice();
    assert_eq!(fields.len(), 4);
    assert_eq!((fields[0].kind, fields[0].row, fields[0].column, fields[0].width), (FieldKind::Text, 3, 7, 31));
    assert_eq!((fields[0].max_length, fields[0].required), (Some(50), true));
    assert_eq!(fields[1].kind, FieldKind::Email);
    assert_eq!(fields[1].max_length, Some(100));
    assert_eq!((fields[2].kind, fields[2].row, fields[2].column), (FieldKind::Radio, 6, 10));
    assert_eq!(fields[3].label.as_bytes(), b"Send message");
    assert_eq!(fields[3].action.as_bytes(), b"send_contact");
    let options = form.options.as_slice();
    assert_eq!(options.len(), 3);
    assert!(options[0].selected && !options[1].selected);
    assert_eq!(options.iter().map(|option| option.column).collect::<Vec<_>>(), [None, Some(19), Some(31)]);
    let (method, address) = form.submission.unwrap();
    assert_eq!((method, address.as_bytes()), (FormMethod::Post, b"/contact".as_slice()));
    let (success, error) = form.result_pages.unwrap();
    assert_eq!((success.mpp(), error.mpp()), (0x401, 0x401));
    assert_eq!(form.token.as_bytes(), b"5f2c9a17e0");
}

/// REQ-WEB-010, REQ-INP-013: the form commands never reach the packets, so
/// the page displays identically on a decoder without the web module.
#[test]
fn form_commands_leave_the_transmitted_page_unchanged() {
    let form_command = |line: &[u8]| {
        [b"FT".as_slice(), b"FD", b"FS", b"FP", b"OPTION"].iter().any(|command| line.starts_with(command))
    };
    let stripped: Vec<u8> = FORM_EXAMPLE
        .split_inclusive(|&byte| byte == b'\n')
        .filter(|line| !form_command(line))
        .flatten()
        .copied()
        .collect();
    // FT, four FD, three OPTION, FS and FP.
    assert_eq!(FORM_EXAMPLE.split(|&byte| byte == b'\n').filter(|line| form_command(line)).count(), 10);
    let packets = |text: &[u8]| {
        let (pages, diagnostics) = read(text);
        assert_eq!(diagnostics, []);
        let mut out = Vec::new();
        encode_page(&pages[0], &mut |packet| out.push(packet));
        (pages[0].form.fields.len(), out)
    };
    let (with_fields, with_form) = packets(FORM_EXAMPLE);
    let (without_fields, without_form) = packets(&stripped);
    assert_eq!((with_fields, without_fields), (4, 0));
    assert_eq!(with_form, without_form);
}

#[test]
fn form_problems_are_recorded() {
    // Version 2.0 syntax: no column and width.
    let (pages, _) = read(b"PN,40001\nOL,3,Name: ....\nFD,TEXT,name,50,Y\n");
    assert!(pages[0].has_issue(IssueKind::FormNeedsConversion));
    assert!(!pages[0].form.interactive);
    // Overlapping fields, a field off the row, the withdrawn FH, an option
    // with no field.
    let text = b"PN,40001\nOPTION,a,b,\nOL,3,x\nFD,TEXT,a,5,10,,N,,\nFD,TEXT,b,10,10,,N,,\nFD,TEXT,c,35,10,,N,,\nFH,X-Test,1\n";
    let (pages, diagnostics) = read(text);
    let page = &pages[0];
    for kind in [
        IssueKind::OptionWithoutField,
        IssueKind::FieldOverlap,
        IssueKind::FieldOutsideRow,
        IssueKind::WithdrawnCommand,
    ] {
        assert!(page.has_issue(kind), "{kind:?}");
    }
    assert!(!page.form.interactive);
    let overlap = page.issues.as_slice().iter().find(|issue| issue.kind == IssueKind::FieldOverlap).unwrap();
    assert_eq!(overlap.line, 5);
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.kind == DiagnosticKind::Page(IssueKind::WithdrawnCommand)));
    // Radio markers that collide.
    let text = b"PN,40001\nOL,6,( ) a ( ) b\nFD,RADIO,r,0,3,N\nOPTION,a,a,\nOPTION,b,b,,1\n";
    assert!(read(text).0[0].has_issue(IssueKind::FieldOverlap));
}
