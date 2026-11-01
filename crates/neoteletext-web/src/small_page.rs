//! What a web browser and a search engine are shown of a channel: the small
//! page of DEC-1669 (neoteletext-common), at a channel's address and at each
//! place in it, on the server and as a static channel's `index.html`
//! (REQ-1725), and a public page's answer (REQ-1728). One builder, so that a
//! live channel and a static one cannot differ. Feature `alloc`.
//!
//! **Everything given here is untrusted text.** Nothing is ever taken as
//! markup: each value is escaped for where it goes, for HTML by `escaped` and
//! for the JSON-LD block by `json_text`, so a program cannot put a script,
//! a tag or a closing `</script>` in the page.
//!
//! **The hidden text** follows Google's structured data for paywalled
//! content, read on 1 November 2026 (last updated 8 September 2026):
//! <https://developers.google.com/search/docs/appearance/structured-data/paywalled-content>.
//! The element is named by a class because that page accepts only a `.class`
//! selector, the object is of a type that page lists, and both it and `hasPart`
//! carry `isAccessibleForFree` false. The same bytes go to everyone, so this
//! is the sanctioned form of hidden text and not cloaking.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use core::fmt::Write;

use neoteletext_core::consts::LINK_SCHEME;

use crate::base64;

/// Where someone without the browser gets it.
pub const GET_NEOTELETEXT: &str = "https://www.neoteletext.com/";
/// The class of the element that holds a public page's text.
pub const HIDDEN_TEXT_CLASS: &str = "paywall";
/// The selector of [`HIDDEN_TEXT_CLASS`], which the JSON-LD names.
pub const HIDDEN_TEXT_SELECTOR: &str = ".paywall";
/// The one rule of a public page's stylesheet: the selector, hidden.
pub const STYLESHEET: &str = ".paywall{display:none}";
/// The type of a public page in its JSON-LD: `WebPage`, because a page of a
/// channel may be anything, and that page lists it.
const PAGE_TYPE: &str = "WebPage";

/// The small page's `Content-Security-Policy`: it loads and runs nothing. The
/// server sees the whole address, so the page's link needs no script
/// (DEC-1694).
pub const CONTENT_SECURITY_POLICY: &str =
    "default-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
/// A public page's: the same, and an inline picture, and the one stylesheet of
/// [`STYLESHEET`], named by its SHA-256 in base64 (CSP Level 3 section 8.2)
/// because `default-src 'none'` refuses every inline style. Nothing a program
/// gives can change those bytes, so this allows no style from a program.
pub const PUBLIC_CONTENT_SECURITY_POLICY: &str = "default-src 'none'; img-src data:; style-src 'sha256-CQDke1K2WQBB0BV70zICx4wzPWIlsqWqs9S/rIuryuQ='; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

/// What a public page shows. Its title and text are the program's, untrusted.
#[derive(Clone, Copy, Debug)]
pub struct PublicPage<'a> {
    /// The page's title.
    pub title: &'a str,
    /// The page's text, for the hidden element.
    pub text: &'a str,
    /// The page's own address, which search engines take for it: a page's
    /// `<channel>NNN`, whatever sub-page or query was asked for.
    pub canonical: &'a str,
    /// The picture, as a PNG, and its width and height in pixels; none when
    /// the host could not draw one.
    pub picture: Option<(&'a [u8], usize, usize)>,
}

/// `text` safe in HTML, in a tag's text or in an attribute in double quotes.
fn escaped(text: &str) -> String {
    let mut written = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => written.push_str("&amp;"),
            '<' => written.push_str("&lt;"),
            '>' => written.push_str("&gt;"),
            '"' => written.push_str("&quot;"),
            '\'' => written.push_str("&#39;"),
            other => written.push(other),
        }
    }
    written
}

/// `character` as a JSON `\uXXXX` escape (RFC 8259 section 7): each is in the
/// Basic Multilingual Plane, so one escape holds it.
fn push_escape(written: &mut String, character: char) {
    let _ = write!(written, "\\u{:04x}", u32::from(character));
}

/// `text` as a JSON string, safe inside a `<script>` block: RFC 8259 section 7,
/// and `<`, `>` and `&` written as escapes, so that it cannot hold `</script>`
/// or `<!--`, which end or change the block whatever JSON says of them (HTML
/// standard section 4.12.1.3).
fn json_text(text: &str) -> String {
    let mut written = String::with_capacity(text.len().saturating_add(2));
    written.push('"');
    for character in text.chars() {
        match character {
            '"' => written.push_str("\\\""),
            '\\' => written.push_str("\\\\"),
            '<' | '>' | '&' | '\u{2028}' | '\u{2029}' => push_escape(&mut written, character),
            control if control < ' ' => push_escape(&mut written, control),
            other => written.push(other),
        }
    }
    written.push('"');
    written
}

/// The document around `head` and `body`, its title `title`, already escaped.
fn document(title: &str, head: &str, body: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title}</title>\n{head}</head>\n\
         <body>\n{body}</body>\n</html>\n"
    )
}

/// The link that opens `address` in the browser and the link to get it.
fn offer(address: &str) -> String {
    let link = escaped(&format!("{LINK_SCHEME}{address}"));
    format!(
        "<p><a id=\"open\" href=\"{link}\">Open in NeoTeletext</a></p>\n<p><a href=\"{GET_NEOTELETEXT}\">Get NeoTeletext</a></p>\n"
    )
}

/// The small page's body: the channel's name and the offer.
fn small_body(name: &str, address: &str) -> String {
    format!(
        "<h1>{name}</h1>\n<p>This is a teletext channel, which opens in the NeoTeletext browser.</p>\n{}",
        offer(address)
    )
}

/// The small page at a channel's own address, its place of `contents` or of
/// `wall`: the channel's name, a link that opens `address` in the browser,
/// and a link to get the browser. Search engines may list it.
#[must_use]
pub fn channel_page(name: &str, address: &str) -> String {
    let name = escaped(name);
    document(&name, "", &small_body(&name, address))
}

/// What a page's address answers when the page is not public, does not exist
/// or was not described in time: the small page, and `noindex`. It holds
/// nothing a program gave, so that the address says nothing of whether a
/// private page exists (REQ-1728 item 3); it differs only in `address`, which
/// repeats what the visitor already holds.
#[must_use]
pub fn private_page(name: &str, address: &str) -> String {
    let name = escaped(name);
    document(&name, "<meta name=\"robots\" content=\"noindex\">\n", &small_body(&name, address))
}

/// The JSON-LD that says the hidden text is there, and which element holds it.
fn structured_data(title: &str, canonical: &str) -> String {
    format!(
        "{{\"@context\":\"https://schema.org\",\"@type\":\"{PAGE_TYPE}\",\"url\":{},\"headline\":{},\"isAccessibleForFree\":false,\
         \"hasPart\":{{\"@type\":\"WebPageElement\",\"isAccessibleForFree\":false,\"cssSelector\":\"{HIDDEN_TEXT_SELECTOR}\"}}}}",
        json_text(canonical),
        json_text(title)
    )
}

/// The inline image of `png`, a page's picture (REQ-1728 item 4): inside the
/// answer, so that no new address is added to the URL map.
fn image(png: &[u8], width: usize, height: usize, title: &str) -> String {
    let mut encoded = vec![0_u8; base64::encoded_len(png.len())];
    // The buffer is the encoded length, so this cannot fail, and base64's
    // alphabet needs no escaping.
    let written = base64::encode(png, &mut encoded).unwrap_or(0);
    let encoded = String::from_utf8_lossy(encoded.get(..written).unwrap_or_default());
    format!(
        "<p><img src=\"data:image/png;base64,{encoded}\" width=\"{width}\" height=\"{height}\" alt=\"{title}\"></p>\n"
    )
}

/// What a public page's address answers (REQ-1728 items 4 and 5): its title,
/// its picture and the offer to open it, with its text in the element the
/// stylesheet hides and the JSON-LD declares. `address` is what was asked for,
/// which the link repeats.
#[must_use]
pub fn public_page(name: &str, address: &str, page: &PublicPage<'_>) -> String {
    let (name, title, text, canonical) =
        (escaped(name), escaped(page.title), escaped(page.text), escaped(page.canonical));
    let head = format!(
        "<link rel=\"canonical\" href=\"{canonical}\">\n<style>{STYLESHEET}</style>\n<script type=\"application/ld+json\">{}</script>\n",
        structured_data(page.title, page.canonical)
    );
    let picture = page.picture.map_or_else(String::new, |(png, width, height)| image(png, width, height, &title));
    let body = format!(
        "<h1>{title}</h1>\n<p>A page of {name}, a teletext channel, which opens in the NeoTeletext browser.</p>\n{picture}{}\
         <div class=\"{HIDDEN_TEXT_CLASS}\">{text}</div>\n",
        offer(address)
    );
    document(&title, &head, &body)
}

#[cfg(test)]
mod tests;
