//! `neottx html` (neoteletext-common's REQ-1725 verification items 2 and 3,
//! REQ-1728 for a page a static channel opts in): a static channel's folder
//! gets an `index.html` and an `NNN/index.html` for each page, which hold
//! nothing of the page by default, and which change nothing a reader does.
#![allow(missing_docs)]

mod common;

use common::neottx;
use std::path::{Path, PathBuf};

const ADDRESS: &str = "https://h.example/news/";

/// A channel's folder of two pages and its entry file.
fn channel(name: &str) -> PathBuf {
    let folder = common::folder(name);
    let write = |name: &str, text: &str| std::fs::write(folder.join(name), text).expect("write a test file");
    write("channel.ttx", r#"{"name":"News <of> the day","initial_page":"100"}"#);
    write("100.tti", "DE,The index\r\nPN,10000\r\nOL,1,SECRET INDEX WORDS\r\n");
    write("101.tti", "DE,Top story\r\nPN,10100\r\nOL,1,THE TOP STORY TODAY\r\nOL,2,AND MORE OF IT\r\n");
    folder
}

fn html(folder: &Path, public: &[&str]) -> std::process::Output {
    let mut arguments = vec!["html", folder.to_str().unwrap(), "--address", ADDRESS];
    for page in public {
        arguments.extend(["--public", page]);
    }
    neottx(&arguments)
}

fn read(folder: &Path, name: &str) -> String {
    std::fs::read_to_string(folder.join(name)).expect("the file was written")
}

/// REQ-1725 verification 2: the channel's page and a page for each page, with
/// the channel's name and the link.
#[test]
fn a_folder_gets_a_page_for_the_channel_and_for_each_page() {
    let folder = channel("html-default");
    let output = html(&folder, &[]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let index = read(&folder, "index.html");
    assert!(index.contains("<h1>News &lt;of&gt; the day</h1>"), "{index}");
    assert!(index.contains("href=\"neottx://https://h.example/news/\""));
    assert!(!index.contains("noindex"));
    for page in ["100", "101"] {
        let shown = read(&folder, &format!("{page}/index.html"));
        assert!(shown.contains("News &lt;of&gt; the day"));
        assert!(shown.contains(&format!("href=\"neottx://https://h.example/news/{page}\"")), "{shown}");
    }
}

/// The default holds no content of the page: not its title, nor text, nor a
/// picture, and search engines are told to leave it.
#[test]
fn a_page_shows_nothing_of_itself_by_default() {
    let folder = channel("html-private");
    assert!(html(&folder, &[]).status.success());
    let shown = read(&folder, "101/index.html");
    for held in ["Top story", "TOP STORY", "<img", "ld+json", "paywall"] {
        assert!(!shown.contains(held), "{held}");
    }
    assert!(shown.contains("noindex"));
}

/// Only a page the channel names carries its picture, title and hidden text.
#[test]
fn a_page_the_channel_names_shows_what_a_public_page_shows() {
    let folder = channel("html-public");
    let output = html(&folder, &["101"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let shown = read(&folder, "101/index.html");
    assert!(shown.contains("<h1>Top story</h1>"), "{shown}");
    assert!(shown.contains("<img src=\"data:image/png;base64,iVBOR"));
    assert!(shown.contains("<div class=\"paywall\">THE TOP STORY TODAY AND MORE OF IT</div>"));
    assert!(shown.contains("\"isAccessibleForFree\":false"));
    assert!(!shown.contains("noindex"));
    assert!(!read(&folder, "100/index.html").contains("SECRET"), "the page not named stays private");
}

#[test]
fn a_public_page_the_folder_lacks_is_refused() {
    let folder = channel("html-absent");
    let output = html(&folder, &["222"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("222"));
    let output = html(&folder, &["not-a-page"]);
    assert!(!output.status.success());
}

#[test]
fn an_address_that_is_no_channels_is_refused() {
    let folder = channel("html-address");
    let output = neottx(&["html", folder.to_str().unwrap(), "--address", "https://h.example/news"]);
    assert!(!output.status.success());
    assert!(!folder.join("index.html").exists());
}

/// REQ-1725 verification 3: the files change nothing a reader or author does.
/// A reader opens a page by its fixed name, and `contents` lists the same.
#[test]
fn a_channel_reads_the_same_with_its_web_pages() {
    let folder = channel("html-reader");
    let contents = || {
        let output = neottx(&["contents", folder.to_str().unwrap()]);
        assert!(output.status.success());
        output.stdout
    };
    let page = || {
        let output = neottx(&["text", folder.join("101.tti").to_str().unwrap(), "101"]);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        output.stdout
    };
    let (before, shown) = (contents(), page());
    assert!(html(&folder, &["100"]).status.success());
    assert_eq!(contents(), before);
    assert_eq!(page(), shown);
    assert_eq!(neoteletext_web::channel::ChannelFile::from_name("index.html"), None);
}
