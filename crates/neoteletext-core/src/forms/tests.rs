//! Tests of the form model (Appendix C): the specification, editing from a keyboard and a remote control, validation, the submitted pairs and drawing.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use crate::charset::to_unicode;
use crate::level1::{Glyph, Grid};

use super::validation::{valid_date, valid_email, valid_number, valid_time};
use crate::charset::{G0Set, NationalOption};

fn name(text: &[u8]) -> Name {
    let mut name = FixedVec::new(0);
    for &b in text {
        name.push(b);
    }
    name
}

fn spec() -> FormSpec {
    let mut fields = FixedVec::new(FieldSpec::new(FieldKind::Text, 0, 0, 1));
    let mut text = FieldSpec::new(FieldKind::Text, 3, 7, 10);
    text.name = name(b"name");
    text.required = true;
    fields.push(text);
    let mut email = FieldSpec::new(FieldKind::Email, 4, 7, 20);
    email.name = name(b"email");
    fields.push(email);
    let mut tick = FieldSpec::new(FieldKind::Checkbox, 5, 2, 1);
    tick.name = name(b"news");
    tick.value = name(b"yes");
    fields.push(tick);
    let mut send = FieldSpec::new(FieldKind::Submit, 8, 2, 8);
    send.action = name(b"send");
    fields.push(send);
    FormSpec {
        fields,
        options: FixedVec::new(OptionSpec {
            value: name(b""),
            display: FixedVec::new(0),
            selected: false,
            column: None,
        }),
        method: Method::Post,
        target: FixedVec::new(0),
        success: None,
        error: None,
        token: FixedVec::new(0),
    }
}

fn english() -> G0 {
    G0 { set: G0Set::Latin, national: Some(NationalOption::English) }
}

#[test]
fn overlapping_fields_are_refused() {
    let mut bad = spec();
    bad.fields.push(FieldSpec::new(FieldKind::Text, 3, 10, 5));
    assert_eq!(FormModel::new(bad, english()).err(), Some(FormError::Overlap));
}

/// REQ-WEB-010: client-side validation.
#[test]
fn keyboard_entry_validation_and_pairs() {
    let mut form = FormModel::new(spec(), english()).unwrap();
    let now = Instant(0);
    assert_eq!(form.key(FormKey::Cyan, now), FormAction::Invalid(0, Invalid::Required));
    for c in "Ann".chars() {
        form.key(FormKey::Char(c), now);
    }
    form.key(FormKey::Tab, now);
    for c in "a@b".chars() {
        form.key(FormKey::Char(c), now);
    }
    form.key(FormKey::Tab, now);
    form.key(FormKey::Space, now);
    form.key(FormKey::Tab, now);
    assert_eq!(form.key(FormKey::Enter, now), FormAction::Submit(Some(3)));
    let mut seen = std::vec::Vec::new();
    form.pairs(&mut |name, value| seen.push((name.to_vec(), value.iter().collect::<std::string::String>())));
    assert_eq!(seen[0], (b"name".to_vec(), "Ann".into()));
    assert_eq!(seen[1], (b"email".to_vec(), "a@b".into()));
    assert_eq!(seen[2], (b"news".to_vec(), "yes".into()));
}

#[test]
fn characters_the_set_cannot_show_are_refused() {
    let mut form = FormModel::new(spec(), english()).unwrap();
    // English G0 has no "#" at 2/3 (it is "£"), and no "é" at all.
    assert_eq!(form.key(FormKey::Char('é'), Instant(0)), FormAction::Refused);
    assert_eq!(form.key(FormKey::Char('£'), Instant(0)), FormAction::None);
}

#[test]
fn remote_multitap_letters() {
    let mut form = FormModel::new(spec(), english()).unwrap();
    form.key(FormKey::RemoteDigit(2), Instant(0));
    form.key(FormKey::RemoteDigit(2), Instant(100_000));
    form.key(FormKey::RemoteDigit(2), Instant(200_000));
    form.key(FormKey::RemoteDigit(3), Instant(300_000));
    form.key(FormKey::RemoteDigit(3), Instant(2_000_000));
    let value: std::vec::Vec<u8> = form.value(0).to_vec();
    assert_eq!(value, b"CDD".to_vec());
    form.key(FormKey::Yellow, Instant(3_000_000));
    form.key(FormKey::RemoteDigit(7), Instant(3_100_000));
    assert_eq!(form.value(0), b"CDD7");
}

#[test]
fn number_date_and_time_rules() {
    let mut field = FieldSpec::new(FieldKind::Number, 0, 0, 5);
    field.min = 0;
    field.max = 100;
    field.step = 5;
    assert!(valid_number(b"35", &field));
    assert!(!valid_number(b"36", &field));
    assert!(!valid_number(b"105", &field));
    let date = FieldSpec::new(FieldKind::Date, 0, 0, 10);
    assert!(valid_date(b"2026-10-10", &date));
    assert!(!valid_date(b"2026-13-10", &date));
    // Days per month, with the Gregorian leap years.
    assert!(!valid_date(b"2026-02-31", &date) && !valid_date(b"2026-04-31", &date));
    assert!(valid_date(b"2026-01-31", &date) && valid_date(b"2024-02-29", &date));
    assert!(!valid_date(b"2026-02-29", &date) && !valid_date(b"1900-02-29", &date));
    assert!(valid_date(b"2000-02-29", &date) && !valid_date(b"2026-02-00", &date));
    assert!(valid_time(b"23:59", &date));
    assert!(!valid_time(b"24:00", &date));
    assert!(valid_email(b"a@b") && !valid_email(b"@b") && !valid_email(b"a@b@c"));
}

const NOW: Instant = Instant(0);

fn fixed<const N: usize>(text: &[u8]) -> FixedVec<u8, N> {
    let mut out = FixedVec::new(0);
    for &b in text {
        assert!(out.push(b));
    }
    out
}

fn choice(value: &[u8], display: &[u8], selected: bool, column: Option<u8>) -> OptionSpec {
    OptionSpec { value: fixed(value), display: fixed(display), selected, column }
}

fn form_spec(fields: &[FieldSpec], options: &[OptionSpec]) -> FormSpec {
    let mut spec = spec();
    spec.fields.clear();
    for &field in fields {
        assert!(spec.fields.push(field));
    }
    for &option in options {
        assert!(spec.options.push(option));
    }
    spec
}

fn form(fields: &[FieldSpec], options: &[OptionSpec]) -> FormModel {
    FormModel::new(form_spec(fields, options), english()).unwrap()
}

fn type_text(form: &mut FormModel, text: &str) {
    for c in text.chars() {
        assert_eq!(form.key(FormKey::Char(c), NOW), FormAction::None, "typing {c:?}");
    }
}

/// Validates `text` typed into a form holding only `field`.
fn check(field: &FieldSpec, text: &str) -> Result<(), Invalid> {
    let mut form = form(&[*field], &[]);
    type_text(&mut form, text);
    form.validate().map_err(|(index, reason)| {
        assert_eq!(index, 0);
        reason
    })
}

fn submitted(form: &FormModel) -> std::vec::Vec<(std::string::String, std::string::String)> {
    let mut seen = std::vec::Vec::new();
    form.pairs(&mut |name, value| {
        seen.push((std::string::String::from_utf8(name.to_vec()).unwrap(), value.iter().collect()));
    });
    seen
}

fn pair(name: &str, value: &str) -> (std::string::String, std::string::String) {
    (name.into(), value.into())
}

/// Appendix C.2: a field must fit within the page, and its options must
/// exist.
#[test]
fn fields_outside_the_page_or_options_are_refused() {
    let past_right = FieldSpec::new(FieldKind::Text, 1, 35, 6);
    let mut past_bottom = FieldSpec::new(FieldKind::TextArea, 23, 0, 10);
    past_bottom.rows = 3;
    let zero_width = FieldSpec::new(FieldKind::Text, 1, 0, 0);
    for field in [past_right, past_bottom, zero_width] {
        assert_eq!(FormModel::new(form_spec(&[field], &[]), english()).err(), Some(FormError::OutOfPage), "{field:?}");
    }
    // The last cell of the page is inside it.
    let last = FieldSpec::new(FieldKind::Text, 24, 39, 1);
    assert!(FormModel::new(form_spec(&[last], &[]), english()).is_ok());
    let mut select = FieldSpec::new(FieldKind::Select, 2, 0, 5);
    select.option_count = 2;
    let one = [choice(b"a", b"A", false, None)];
    assert_eq!(FormModel::new(form_spec(&[select], &one), english()).err(), Some(FormError::BadOptions));
}

#[test]
fn a_new_form_starts_at_the_first_field_in_letter_mode() {
    let form = FormModel::new(spec(), english()).unwrap();
    assert_eq!(form.focus(), 0);
    assert_eq!(form.mode(), EntryMode::Letters);
    assert_eq!(form.spec().fields.len(), 4);
    assert_eq!(form.spec().method, Method::Post);
}

/// REQ-NAV-009: Tab and Green move forward, Shift-Tab and Red back,
/// both wrapping; Index leaves.
#[test]
fn focus_moves_both_ways_and_wraps() {
    let mut form = FormModel::new(spec(), english()).unwrap();
    let mut focus = std::vec::Vec::new();
    for key in [FormKey::BackTab, FormKey::Red, FormKey::Green, FormKey::Tab, FormKey::Tab] {
        assert_eq!(form.key(key, NOW), FormAction::None);
        focus.push(form.focus());
    }
    assert_eq!(focus, [3, 2, 3, 0, 1]);
    assert_eq!(form.key(FormKey::Index, NOW), FormAction::Leave);
}

/// A form with no fields has nothing to focus or edit, and submits.
#[test]
fn an_empty_form_only_submits_or_leaves() {
    let mut form = form(&[], &[]);
    for key in [FormKey::Tab, FormKey::BackTab, FormKey::Char('a'), FormKey::Up, FormKey::Backspace, FormKey::Space] {
        assert_eq!(form.key(key, NOW), FormAction::None, "{key:?}");
        assert_eq!(form.focus(), 0);
    }
    assert_eq!(form.key(FormKey::Enter, NOW), FormAction::Submit(None));
    assert_eq!(form.key(FormKey::Index, NOW), FormAction::Leave);
    assert_eq!(submitted(&form), []);
}

/// REQ-NAV-009: typing inserts at the cursor, the arrows move it within
/// the value, Backspace deletes before it, and the space bar types a
/// space in a text field.
#[test]
fn cursor_editing() {
    let mut text = FieldSpec::new(FieldKind::Text, 1, 0, 20);
    text.name = name(b"t");
    let mut form = form(&[text], &[]);
    type_text(&mut form, "ac");
    form.key(FormKey::Left, NOW);
    type_text(&mut form, "b");
    assert_eq!(form.value(0), b"abc");
    // Right stops at the end of the value; Left stops at the start.
    for _ in 0..5 {
        form.key(FormKey::Right, NOW);
    }
    assert_eq!(form.key(FormKey::Space, NOW), FormAction::None);
    type_text(&mut form, "d");
    assert_eq!(form.value(0), b"abc d");
    for _ in 0..9 {
        form.key(FormKey::Left, NOW);
    }
    assert_eq!(form.key(FormKey::Backspace, NOW), FormAction::None);
    assert_eq!(form.value(0), b"abc d", "nothing before the cursor");
    form.key(FormKey::Right, NOW);
    form.key(FormKey::Right, NOW);
    form.key(FormKey::Backspace, NOW);
    assert_eq!(form.value(0), b"ac d");
    type_text(&mut form, "B");
    assert_eq!(form.value(0), b"aBc d");
    assert_eq!(submitted(&form), [pair("t", "aBc d")]);
}

/// Appendix C.5: `maxlen` stops entry at the limit.
#[test]
fn entry_stops_at_maxlen() {
    let mut text = FieldSpec::new(FieldKind::Text, 1, 0, 20);
    text.maxlen = 3;
    let mut form = form(&[text], &[]);
    type_text(&mut form, "abc");
    assert_eq!(form.key(FormKey::Char('d'), NOW), FormAction::Refused);
    assert_eq!(form.value(0), b"abc");
}

/// Characters go only into fields that take text.
#[test]
fn typing_into_a_marker_or_button_does_nothing() {
    let tick = FieldSpec::new(FieldKind::Checkbox, 1, 0, 1);
    let mut form = form(&[tick, FieldSpec::new(FieldKind::Submit, 2, 0, 4)], &[]);
    assert_eq!(form.key(FormKey::Char('x'), NOW), FormAction::None);
    assert_eq!(form.key(FormKey::RemoteDigit(5), NOW), FormAction::None);
    assert_eq!(form.value(0), b"");
    form.key(FormKey::Tab, NOW);
    assert_eq!(form.key(FormKey::Char('x'), NOW), FormAction::None);
    assert_eq!(form.value(1), b"");
}

fn colours() -> ([FieldSpec; 3], [OptionSpec; 5]) {
    let mut colour = FieldSpec::new(FieldKind::Select, 2, 10, 5);
    colour.name = name(b"colour");
    colour.option_count = 3;
    let mut size = FieldSpec::new(FieldKind::Radio, 6, 10, 1);
    size.name = name(b"size");
    size.first_option = 3;
    size.option_count = 2;
    let mut tick = FieldSpec::new(FieldKind::Checkbox, 8, 2, 1);
    tick.name = name(b"news");
    tick.value = name(b"yes");
    let options = [
        choice(b"r", b"Red", false, None),
        choice(b"g", b"Green", true, None),
        choice(b"b", b"Blue", false, None),
        choice(b"s", b"Small", false, None),
        choice(b"l", b"Large", false, Some(20)),
    ];
    ([colour, size, tick], options)
}

/// REQ-NAV-009: the arrows and Yellow step SELECT and RADIO choices,
/// wrapping; the default is the option marked selected, else the first.
#[test]
fn choices_step_and_wrap() {
    let (fields, options) = colours();
    let mut form = form(&fields, &options);
    assert_eq!(submitted(&form), [pair("colour", "g"), pair("size", "s")]);
    let mut seen = std::vec::Vec::new();
    for key in [FormKey::Down, FormKey::Down, FormKey::Up, FormKey::Up, FormKey::Up, FormKey::Yellow] {
        form.key(key, NOW);
        seen.push(submitted(&form)[0].1.clone());
    }
    assert_eq!(seen, ["b", "r", "b", "g", "r", "g"]);
    form.key(FormKey::Tab, NOW);
    form.key(FormKey::Yellow, NOW);
    assert_eq!(submitted(&form)[1], pair("size", "l"));
    form.key(FormKey::Yellow, NOW);
    assert_eq!(submitted(&form)[1], pair("size", "s"));
    form.key(FormKey::Up, NOW);
    assert_eq!(submitted(&form)[1], pair("size", "l"));
    // Arrows on a field with no choices change nothing.
    form.key(FormKey::Tab, NOW);
    form.key(FormKey::Down, NOW);
    assert_eq!(submitted(&form), [pair("colour", "g"), pair("size", "l")]);
}

/// REQ-NAV-009: the space bar and Yellow toggle a checkbox, which is
/// submitted only when checked (REQ-WEB-011).
#[test]
fn checkbox_toggles() {
    let (fields, options) = colours();
    let mut form = form(&fields, &options);
    form.key(FormKey::BackTab, NOW);
    assert_eq!(form.focus(), 2);
    form.key(FormKey::Space, NOW);
    assert_eq!(submitted(&form)[2], pair("news", "yes"));
    form.key(FormKey::Yellow, NOW);
    assert_eq!(submitted(&form).len(), 2);
    form.key(FormKey::Yellow, NOW);
    assert_eq!(submitted(&form)[2], pair("news", "yes"));
}

/// Appendix C.3: RESET restores every field to its default.
#[test]
fn reset_restores_defaults() {
    let (fields, options) = colours();
    let mut text = FieldSpec::new(FieldKind::Text, 10, 0, 10);
    text.name = name(b"t");
    text.default = fixed(b"Hi");
    let reset = FieldSpec::new(FieldKind::Reset, 12, 0, 7);
    let mut checked = fields[2];
    checked.checked = true;
    let mut form = form(&[text, fields[0], fields[1], checked, reset], &options);
    let defaults = submitted(&form);
    assert_eq!(defaults, [pair("t", "Hi"), pair("colour", "g"), pair("size", "s"), pair("news", "yes")]);
    // Coming back to a field puts the cursor after its value.
    form.key(FormKey::BackTab, NOW);
    form.key(FormKey::Tab, NOW);
    type_text(&mut form, "!");
    assert_eq!(form.value(0), b"Hi!");
    form.key(FormKey::Tab, NOW);
    form.key(FormKey::Down, NOW);
    form.key(FormKey::Tab, NOW);
    form.key(FormKey::Down, NOW);
    form.key(FormKey::Tab, NOW);
    form.key(FormKey::Space, NOW);
    assert_ne!(submitted(&form), defaults);
    form.key(FormKey::Tab, NOW);
    assert_eq!(form.key(FormKey::Enter, NOW), FormAction::None);
    assert_eq!(submitted(&form), defaults);
    assert_eq!(form.focus(), 4);
}

/// REQ-NAV-009: Yellow switches text entry between letters and digits,
/// and back.
#[test]
fn yellow_switches_entry_mode() {
    let mut form = form(&[FieldSpec::new(FieldKind::Text, 1, 0, 10)], &[]);
    form.key(FormKey::Yellow, NOW);
    assert_eq!(form.mode(), EntryMode::Digits);
    form.key(FormKey::RemoteDigit(4), NOW);
    form.key(FormKey::Yellow, NOW);
    assert_eq!(form.mode(), EntryMode::Letters);
    form.key(FormKey::RemoteDigit(4), Instant(1));
    assert_eq!(form.value(0), b"4G");
    // There is no digit key 10.
    assert_eq!(form.key(FormKey::RemoteDigit(10), Instant(2)), FormAction::None);
    assert_eq!(form.value(0), b"4G");
}

/// Appendix C.5 and REQ-WEB-011: a required field must be filled; an
/// empty optional field passes whatever its type.
#[test]
fn required_and_empty_fields() {
    for kind in [FieldKind::Text, FieldKind::Email, FieldKind::Tel, FieldKind::Number, FieldKind::Date, FieldKind::Time]
    {
        let mut field = FieldSpec::new(kind, 1, 0, 10);
        assert_eq!(check(&field, ""), Ok(()), "{kind:?}");
        field.required = true;
        assert_eq!(check(&field, ""), Err(Invalid::Required), "{kind:?}");
    }
}

/// Appendix C.5: a default longer than `maxlen` fails.
#[test]
fn value_longer_than_maxlen_fails() {
    let mut field = FieldSpec::new(FieldKind::Text, 1, 0, 10);
    field.maxlen = 3;
    field.default = fixed(b"abcd");
    assert_eq!(form(&[field], &[]).validate(), Err((0, Invalid::TooLong)));
    field.default = fixed(b"abc");
    assert_eq!(form(&[field], &[]).validate(), Ok(()));
}

/// Appendix C.3: `9` a digit, `A` a letter, `*` any character; the
/// value may be shorter than the pattern, not longer.
#[test]
fn simple_patterns() {
    let mut field = FieldSpec::new(FieldKind::Text, 1, 0, 10);
    field.pattern = fixed(b"99A*");
    assert_eq!(check(&field, "12b-"), Ok(()));
    assert_eq!(check(&field, "12"), Ok(()));
    assert_eq!(check(&field, "1a"), Err(Invalid::Pattern));
    assert_eq!(check(&field, "123"), Err(Invalid::Pattern));
    assert_eq!(check(&field, "12b-5"), Err(Invalid::Pattern));
}

/// Appendix C.3: one "@" with text on either side.
#[test]
fn email_rule() {
    let field = FieldSpec::new(FieldKind::Email, 1, 0, 20);
    assert_eq!(check(&field, "ann@example.com"), Ok(()));
    for bad in ["ann", "ann@", "@example", "a@b@c"] {
        assert_eq!(check(&field, bad), Err(Invalid::Email), "{bad}");
    }
}

/// Appendix C.3: digits, spaces and "+" only, unless a pattern says
/// otherwise.
#[test]
fn telephone_rule() {
    let mut field = FieldSpec::new(FieldKind::Tel, 1, 0, 20);
    assert_eq!(check(&field, "+44 20 7946 0000"), Ok(()));
    assert_eq!(check(&field, "020-7946"), Err(Invalid::Tel));
    field.pattern = fixed(b"999*9999");
    assert_eq!(check(&field, "020-7946"), Ok(()));
    assert_eq!(check(&field, "+20-7946"), Err(Invalid::Pattern));
}

/// Appendix C.3 and C.5: whole numbers only, in range.
#[test]
fn number_rule() {
    let mut field = FieldSpec::new(FieldKind::Number, 1, 0, 10);
    field.min = -10;
    field.max = 10;
    for good in ["-10", "-5", "0", "10"] {
        assert_eq!(check(&field, good), Ok(()), "{good}");
    }
    for bad in ["-11", "11", "5a", "-", "--5", "1.5", "99999999999"] {
        assert_eq!(check(&field, bad), Err(Invalid::Number), "{bad}");
    }
}

/// Appendix C.3 and C.5: year-month-day, within the limits.
#[test]
fn date_rule() {
    let mut field = FieldSpec::new(FieldKind::Date, 1, 0, 10);
    field.min_text = fixed(b"2026-01-01");
    field.max_text = fixed(b"2026-12-31");
    for good in ["2026-01-01", "2026-06-15", "2026-12-31"] {
        assert_eq!(check(&field, good), Ok(()), "{good}");
    }
    for bad in ["2025-12-31", "2027-01-01", "2026/06/15", "2026-6-15", "2026-00-10", "2026-10-32", "2026-1a-01"] {
        assert_eq!(check(&field, bad), Err(Invalid::DateTime), "{bad}");
    }
}

/// Appendix C.3 and C.5: hours and minutes, within the limits.
#[test]
fn time_rule() {
    let mut field = FieldSpec::new(FieldKind::Time, 1, 0, 10);
    assert_eq!(check(&field, "00:00"), Ok(()));
    for bad in ["9:30", "09:60", "24:00", "0930", "09:3x"] {
        assert_eq!(check(&field, bad), Err(Invalid::DateTime), "{bad}");
    }
    field.min_text = fixed(b"09:00");
    field.max_text = fixed(b"17:00");
    assert_eq!(check(&field, "09:00"), Ok(()));
    assert_eq!(check(&field, "17:00"), Ok(()));
    assert_eq!(check(&field, "08:59"), Err(Invalid::DateTime));
    assert_eq!(check(&field, "17:01"), Err(Invalid::DateTime));
}

/// REQ-WEB-011 step 1: the first failing field takes the focus, with
/// the cursor at the end of its value.
#[test]
fn submit_moves_to_the_first_failing_field() {
    let mut first = FieldSpec::new(FieldKind::Number, 1, 0, 10);
    first.max = 5;
    let second = FieldSpec::new(FieldKind::Email, 2, 0, 10);
    let mut form = form(&[first, second, FieldSpec::new(FieldKind::Submit, 3, 0, 4)], &[]);
    type_text(&mut form, "3");
    form.key(FormKey::Tab, NOW);
    type_text(&mut form, "nobody");
    form.key(FormKey::Tab, NOW);
    assert_eq!(form.key(FormKey::Cyan, NOW), FormAction::Invalid(1, Invalid::Email));
    assert_eq!(form.focus(), 1);
    type_text(&mut form, "@x");
    assert_eq!(form.value(1), b"nobody@x", "typing continues at the end");
    assert_eq!(form.key(FormKey::Cyan, NOW), FormAction::Submit(None));
}

fn shown(grid: &Grid, row: usize, columns: core::ops::Range<usize>) -> std::string::String {
    columns
        .map(|column| match grid.cell(row, column).unwrap().glyph {
            Glyph::Space => ' ',
            Glyph::Char(character) => to_unicode(character).base,
            Glyph::Mosaic { .. } => '?',
        })
        .collect()
}

fn flashing(grid: &Grid, row: usize, columns: core::ops::Range<usize>) -> std::vec::Vec<usize> {
    columns.filter(|&column| grid.cell(row, column).unwrap().flash).collect()
}

/// Appendix C.4: the value is drawn over the placeholder, the focused
/// field in inverse video with the cursor cell flashing, and concealed
/// placeholders are revealed.
#[test]
fn draw_highlights_the_focused_field() {
    use crate::consts::{COLOUR_BLACK, COLOUR_WHITE};
    let mut form = form(&[FieldSpec::new(FieldKind::Text, 2, 5, 5), FieldSpec::new(FieldKind::Text, 3, 5, 5)], &[]);
    type_text(&mut form, "AB");
    let mut grid = Grid::EMPTY;
    for column in 0..10 {
        grid.cells[2][column].conceal = true;
    }
    grid.cells[3][5].flash = true;
    form.draw(&mut grid);
    assert_eq!(shown(&grid, 2, 4..11), " AB    ");
    for column in 5..10 {
        let cell = grid.cell(2, column).unwrap();
        assert_eq!((cell.foreground, cell.background, cell.conceal), (COLOUR_BLACK, COLOUR_WHITE, false));
    }
    assert!(grid.cells[2][4].conceal, "outside the field");
    assert_eq!(flashing(&grid, 2, 0..COLUMNS), [7]);
    // The field without focus keeps the page's colours and flashing.
    let other = grid.cell(3, 5).unwrap();
    assert_eq!((other.foreground, other.background, other.flash), (COLOUR_WHITE, COLOUR_BLACK, true));
}

/// Appendix C.4: a value longer than the field scrolls within it, and
/// the cursor stays visible and marked.
#[test]
fn draw_scrolls_to_keep_the_cursor_visible() {
    let mut form = form(&[FieldSpec::new(FieldKind::Text, 2, 0, 3)], &[]);
    type_text(&mut form, "ABCDE");
    let draw = |form: &FormModel| {
        let mut grid = Grid::EMPTY;
        form.draw(&mut grid);
        (shown(&grid, 2, 0..3), flashing(&grid, 2, 0..3))
    };
    assert_eq!(draw(&form), ("DE ".into(), std::vec![2]));
    for _ in 0..4 {
        form.key(FormKey::Left, NOW);
    }
    assert_eq!(draw(&form), ("ABC".into(), std::vec![1]));
    form.key(FormKey::Right, NOW);
    form.key(FormKey::Right, NOW);
    assert_eq!(draw(&form), ("BCD".into(), std::vec![2]));
}

/// Appendix C.2: a TEXTAREA continues down the following rows.
#[test]
fn draw_textarea_over_rows() {
    let mut area = FieldSpec::new(FieldKind::TextArea, 10, 4, 3);
    area.rows = 2;
    let mut form = form(&[area], &[]);
    type_text(&mut form, "ABCD");
    let mut grid = Grid::EMPTY;
    form.draw(&mut grid);
    assert_eq!((shown(&grid, 10, 4..7), shown(&grid, 11, 4..7)), ("ABC".into(), "D  ".into()));
    assert_eq!((flashing(&grid, 10, 0..COLUMNS), flashing(&grid, 11, 0..COLUMNS)), (std::vec![], std::vec![5]));
}

/// Appendix C.3 and C.4: SELECT shows the chosen option's text, cut to
/// the field; CHECKBOX a marker when checked; buttons their label.
#[test]
fn draw_select_checkbox_and_buttons() {
    let (fields, options) = colours();
    let mut send = FieldSpec::new(FieldKind::Submit, 12, 2, 6);
    send.label = fixed(b"SEND");
    let mut form = form(&[fields[0], fields[2], send], &options);
    let draw = |form: &FormModel| {
        let mut grid = Grid::EMPTY;
        form.draw(&mut grid);
        [shown(&grid, 2, 10..15), shown(&grid, 8, 2..3), shown(&grid, 12, 2..8)]
    };
    assert_eq!(draw(&form), ["Green", " ", "SEND  "]);
    form.key(FormKey::Down, NOW);
    form.key(FormKey::Tab, NOW);
    form.key(FormKey::Space, NOW);
    assert_eq!(draw(&form), ["Blue ", "*", "SEND  "]);
    let mut grid = Grid::EMPTY;
    form.draw(&mut grid);
    assert!(flashing(&grid, 8, 0..COLUMNS).is_empty(), "no cursor in a marker");
}

/// Appendix C.3: a RADIO field's column is the first choice's marker,
/// and each further choice gives its own; the chosen one is marked.
#[test]
fn draw_radio_marks_the_chosen_choice() {
    let mut urgency = FieldSpec::new(FieldKind::Radio, 6, 10, 1);
    urgency.option_count = 3;
    let options = [
        choice(b"low", b"Low", false, None),
        choice(b"medium", b"Medium", true, Some(19)),
        choice(b"high", b"High", false, Some(31)),
    ];
    let mut form = form(&[urgency, FieldSpec::new(FieldKind::Text, 8, 0, 5)], &options);
    let markers = |form: &FormModel| {
        let mut grid = Grid::EMPTY;
        form.draw(&mut grid);
        [10, 19, 31].map(|column| shown(&grid, 6, column..column.saturating_add(1)))
    };
    assert_eq!(markers(&form), [" ", "*", " "]);
    form.key(FormKey::Down, NOW);
    assert_eq!(markers(&form), [" ", " ", "*"]);
    form.key(FormKey::Down, NOW);
    assert_eq!(markers(&form), ["*", " ", " "]);
    // Without focus, the choice is still shown.
    form.key(FormKey::Tab, NOW);
    assert_eq!(markers(&form), ["*", " ", " "]);
}

/// Appendix C.3: a SELECT's choices are its own `OPTION` lines. One with
/// none submits and shows nothing, never another field's option.
#[test]
fn a_choice_field_without_options_has_no_value() {
    let mut empty = FieldSpec::new(FieldKind::Select, 2, 0, 5);
    empty.name = name(b"empty");
    let mut size = FieldSpec::new(FieldKind::Select, 3, 0, 5);
    size.name = name(b"size");
    size.option_count = 2;
    let options = [choice(b"s", b"Small", true, None), choice(b"l", b"Large", false, None)];
    let form = form(&[empty, size], &options);
    assert_eq!(submitted(&form), [pair("size", "s")]);
    let mut grid = Grid::EMPTY;
    form.draw(&mut grid);
    assert_eq!((shown(&grid, 2, 0..5), shown(&grid, 3, 0..5)), ("     ".into(), "Small".into()));
}
