#![allow(clippy::indexing_slicing, clippy::unwrap_used, reason = "a test")]

use super::*;
use crate::sha256;

const ADDRESS: &str = "https://h.example/news/110";

fn public(title: &str, text: &str) -> String {
    public_page(
        "News",
        ADDRESS,
        &PublicPage { title, text, canonical: ADDRESS, picture: Some((b"not really a png", 480, 500)) },
    )
}

/// What lies between `open` and `close` in `page`, the first time.
fn between<'a>(page: &'a str, open: &str, close: &str) -> &'a str {
    let (_, rest) = page.split_once(open).unwrap();
    rest.split_once(close).unwrap().0
}

/// The hash in the policy is of the bytes the page carries: a mismatch would
/// have a browser refuse the stylesheet and show the hidden text.
#[test]
fn the_policy_allows_exactly_the_stylesheet() {
    let digest = sha256::digest(STYLESHEET.as_bytes());
    let mut encoded = vec![0_u8; base64::encoded_len(digest.len())];
    let written = base64::encode(&digest, &mut encoded).unwrap();
    let hash = core::str::from_utf8(&encoded[..written]).unwrap();
    assert!(PUBLIC_CONTENT_SECURITY_POLICY.contains(&format!("style-src 'sha256-{hash}'")));
    assert_eq!(between(&public("t", "x"), "<style>", "</style>"), STYLESHEET);
}

/// A public page's policy is the small page's and only the two things it adds.
#[test]
fn a_public_pages_policy_adds_only_the_image_and_the_stylesheet() {
    let added: vec::Vec<&str> = PUBLIC_CONTENT_SECURITY_POLICY
        .split("; ")
        .filter(|directive| !CONTENT_SECURITY_POLICY.split("; ").any(|kept| kept == *directive))
        .collect();
    assert_eq!(added.len(), 2, "{added:?}");
    assert_eq!(added[0], "img-src data:");
    assert!(added[1].starts_with("style-src 'sha256-"));
    assert!(!PUBLIC_CONTENT_SECURITY_POLICY.contains("unsafe"));
}

/// REQ-1728 verification 2: the selector, the stylesheet and the element agree.
#[test]
fn the_selector_names_the_element_the_stylesheet_hides() {
    let page = public("A title", "The text of the page.");
    assert_eq!(HIDDEN_TEXT_SELECTOR, format!(".{HIDDEN_TEXT_CLASS}"));
    assert_eq!(STYLESHEET, format!("{HIDDEN_TEXT_SELECTOR}{{display:none}}"));
    assert!(page.contains(&format!("\"cssSelector\":\"{HIDDEN_TEXT_SELECTOR}\"")));
    assert_eq!(page.matches(&format!("class=\"{HIDDEN_TEXT_CLASS}\"")).count(), 1);
    assert_eq!(between(&page, "<div class=\"paywall\">", "</div>"), "The text of the page.");
}

/// REQ-1728 verification 2: the page holds its title, the picture and the offer.
/// The offer names the host that serves the site, not one that redirects.
#[test]
fn the_offer_is_on_the_sites_own_host() {
    assert_eq!(GET_NEOTELETEXT, "https://www.neottx.com/");
}

#[test]
fn a_public_page_holds_its_title_its_picture_and_the_offer() {
    let page = public("A title", "x");
    assert_eq!(between(&page, "<title>", "</title>"), "A title");
    assert_eq!(between(&page, "<h1>", "</h1>"), "A title");
    assert!(page.contains(
        "<img src=\"data:image/png;base64,bm90IHJlYWxseSBhIHBuZw==\" width=\"480\" height=\"500\" alt=\"A title\">"
    ));
    assert!(page.contains(&format!("href=\"{LINK_SCHEME}{ADDRESS}\">Open in NeoTeletext")));
    assert!(page.contains(&format!("href=\"{GET_NEOTELETEXT}\">Get NeoTeletext")));
    assert!(page.contains(&format!("<link rel=\"canonical\" href=\"{ADDRESS}\">")));
    assert!(!page.contains("noindex"));
}

/// A page with no picture is still a page, for a host that cannot draw one.
#[test]
fn a_public_page_may_have_no_picture() {
    let page = public_page("News", ADDRESS, &PublicPage { title: "t", text: "x", canonical: ADDRESS, picture: None });
    assert!(!page.contains("<img"));
    assert!(page.contains("isAccessibleForFree"));
}

/// REQ-1728 verification 3: nothing from a program is markup, in HTML or in
/// the JSON-LD block.
#[test]
fn what_a_program_gives_is_never_markup() {
    let hostile = "</script><b>\"x\"&'<!--\\";
    let page = public(hostile, hostile);
    assert!(!page.contains("<b>"));
    assert_eq!(page.matches("</script>").count(), 1, "only the block's own end");
    assert!(!page.contains("<!--"));
    assert!(!page.contains("</div><"), "the text cannot end its element");
    let block = between(&page, "<script type=\"application/ld+json\">", "</script>");
    assert!(!block.contains('<') && !block.contains('>') && !block.contains('&'));
    assert!(block.contains("\\u003c/script\\u003e\\u003cb\\u003e\\\"x\\\"\\u0026'\\u003c!--\\\\"));
}

#[test]
fn a_json_text_has_no_raw_control_or_line_separator() {
    assert_eq!(json_text("a\nb\u{2028}c\td"), "\"a\\u000ab\\u2028c\\u0009d\"");
}

/// A hostile channel name or address is escaped everywhere too.
#[test]
fn a_name_and_an_address_are_escaped() {
    for page in
        [channel_page("<b>", "https://h.example/a?x=\"><i>"), private_page("<b>", "https://h.example/a?x=\"><i>")]
    {
        assert!(!page.contains("<b>") && !page.contains("<i>"));
        assert!(page.contains("&lt;b&gt;"));
    }
}

/// REQ-1728 item 3: not-public differs from the small page by `noindex`, and
/// two not-public answers differ only in the address.
#[test]
fn a_private_page_is_the_small_page_with_noindex() {
    let (one, other) =
        (private_page("News", "https://h.example/news/100"), private_page("News", "https://h.example/news/101"));
    assert!(one.contains("<meta name=\"robots\" content=\"noindex\">"));
    assert_eq!(one.replace("/100", "/101"), other);
    assert_eq!(
        one.replace("<meta name=\"robots\" content=\"noindex\">\n", ""),
        channel_page("News", "https://h.example/news/100")
    );
    assert!(!channel_page("News", "https://h.example/news/").contains("noindex"));
}
