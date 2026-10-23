//! One HTTP/1.1 request to the relay, with a time limit and bounded
//! answers.

use super::{MAX_HEAD, READ_CHUNK, RESPONSE_TIMEOUT, Stream};
use crate::Error;
use neoteletext_web::http::{BodyDecoder, HeadCollector, parse_head};
use neoteletext_web::proxy::Proxy;
use neoteletext_web::url::Url;
use std::fmt::Write as _;

/// The longest response body read: the relay's answers are a few hundred
/// bytes, a configuration at most its 8 KB limit.
pub const MAX_BODY: usize = 64 * 1024;

/// One HTTP/1.1 request with `Connection: close`, answered with its status
/// and body (at most [`MAX_BODY`] bytes).
///
/// # Errors
/// A network failure, or a response that breaks HTTP or a bound.
pub fn request(
    url: &Url,
    method: &str,
    authorization: Option<&str>,
    json: Option<&str>,
    proxy: Option<&Proxy>,
) -> Result<(u16, Vec<u8>), Error> {
    let mut stream = Stream::connect(url, proxy)?;
    let mut head = format!(
        "{method} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: application/json\r\n",
        url.target(),
        host_header(url)
    );
    if let Some(token) = authorization {
        let _ = write!(head, "Authorization: Bearer {token}\r\n");
    }
    let body = json.unwrap_or("");
    if json.is_some() || method == "POST" {
        let _ = write!(head, "Content-Type: application/json\r\nContent-Length: {}\r\n", body.len());
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.set_read_timeout(RESPONSE_TIMEOUT)?;
    let mut collector = HeadCollector::default();
    let mut head = vec![0_u8; MAX_HEAD];
    let mut chunk = vec![0_u8; READ_CHUNK];
    let mut body = Vec::new();
    let mut decoder: Option<BodyDecoder> = None;
    let mut status = 0;
    loop {
        let count =
            stream.read(&mut chunk)?.ok_or_else(|| Error::Network("the relay did not answer in time".into()))?;
        let mut input = chunk.get(..count).unwrap_or(&[]);
        if count == 0 {
            if let Some(decoder) = &mut decoder {
                decoder.finish_at_close().map_err(|_| Error::Protocol("a truncated HTTP body"))?;
                break;
            }
            return Err(Error::Protocol("no HTTP response"));
        }
        if decoder.is_none() {
            let (used, complete) =
                collector.feed(&mut head, input).map_err(|_| Error::Protocol("an HTTP head too large"))?;
            input = input.get(used..).unwrap_or(&[]);
            if !complete {
                continue;
            }
            let parsed = parse_head(head.get(..collector.len()).unwrap_or(&[]), &mut |_| {})
                .map_err(|_| Error::Protocol("a malformed HTTP head"))?;
            status = parsed.status;
            decoder = Some(
                BodyDecoder::new(parsed.framing, MAX_BODY).map_err(|_| Error::Protocol("an HTTP body too large"))?,
            );
        }
        let Some(decoder) = &mut decoder else { continue };
        while !input.is_empty() && !decoder.is_done() {
            let step = decoder.feed(input).map_err(|_| Error::Protocol("a malformed HTTP body"))?;
            body.extend_from_slice(step.data);
            input = input.get(step.consumed..).unwrap_or(&[]);
            if step.consumed == 0 {
                break;
            }
        }
        if decoder.is_done() {
            break;
        }
    }
    stream.close();
    Ok((status, body))
}

/// The `Host` header: the host, and the port when it is not the scheme's.
fn host_header(url: &Url) -> String {
    let default = if url.is_secure() { 443 } else { 80 };
    if url.port() == default { url.host_text().to_owned() } else { format!("{}:{}", url.host_text(), url.port()) }
}
