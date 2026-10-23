//! The detector (REQ-INP-005): signatures first, then the scoring of the
//! headerless binary layouts by how many of their packets have decodable
//! addresses.

use neoteletext_core::consts::{ADDRESS_BYTE_0, ADDRESS_BYTE_1};
use neoteletext_core::ecc::{Status, hamming84_decode};

use crate::consts::{
    BASE64URL_ALPHABET, DUMP_BYTES_24, DUMP_BYTES_25, EP1_SIGNATURE, PES_START_CODE, SLICED_HEADER_BYTES,
    SLICED_RECORD_BYTES, SLICED_TELETEXT_B, STREAM_ID_PRIVATE_1, TS_SYNC_BYTE,
};
use crate::sliced::ByteOrder;
use crate::t42::T42Variant;
use crate::transport::TsPacketSize;

use super::{DETECTION_BYTES, DETECTION_PACKETS, Detection, Format};
/// Sync bytes that must agree before a transport stream is recognised.
const TS_SYNCS: usize = 4;
/// The UTF-8 byte order mark some editors write at the start of text files.
const BYTE_ORDER_MARK: &[u8] = &[0xEF, 0xBB, 0xBF];
/// A binary layout is accepted when at least this fraction of its packets
/// have decodable addresses: two in three. Random bytes give about one in
/// three.
const ACCEPT_NUMERATOR: usize = 2;
const ACCEPT_DENOMINATOR: usize = 3;

fn without_bom(prefix: &[u8]) -> &[u8] {
    prefix.strip_prefix(BYTE_ORDER_MARK).unwrap_or(prefix)
}

/// True when the text begins with an edit.tf address: a hexadecimal digit, a
/// colon and base64url data, possibly after a web address ending in `#`.
fn looks_like_edittf(text: &[u8]) -> bool {
    let line = text.split(|&byte| byte == b'\n').next().unwrap_or(&[]).trim_ascii();
    let address = line
        .iter()
        .rposition(|&byte| byte == b'#')
        .map_or(line, |index| line.get(index.saturating_add(1)..).unwrap_or(&[]));
    matches!(address, [digit, b':', first, ..] if digit.is_ascii_hexdigit() && BASE64URL_ALPHABET.contains(first))
}

/// True when the text begins with a TTI command: two capital letters and a
/// comma.
fn looks_like_tti(text: &[u8]) -> bool {
    let text = text.trim_ascii_start();
    matches!(text, [first, second, b',', ..] if first.is_ascii_uppercase() && second.is_ascii_uppercase())
}

/// The first offset at which a transport stream of `size` keeps its sync
/// bytes.
fn transport_offset(prefix: &[u8], size: usize, complete: bool) -> Option<usize> {
    (0..size).find(|&offset| {
        let mut syncs = 0usize;
        let mut position = offset;
        while let Some(&byte) = prefix.get(position) {
            if byte != TS_SYNC_BYTE {
                return false;
            }
            syncs = syncs.saturating_add(1);
            if syncs == TS_SYNCS {
                return true;
            }
            position = position.saturating_add(size);
        }
        complete && syncs > 0
    })
}

/// True when both packet address bytes can be decoded, and whether both
/// were error-free.
fn address_quality(first: u8, second: u8) -> (bool, bool) {
    let (_, a) = hamming84_decode(first);
    let (_, b) = hamming84_decode(second);
    (a.is_usable() && b.is_usable(), a == Status::Clean && b == Status::Clean)
}

/// Scores one binary layout at one offset: (units with usable addresses,
/// units with clean addresses, units examined).
fn score_t42(prefix: &[u8], variant: T42Variant, offset: usize) -> (usize, usize, usize) {
    let mut usable = 0usize;
    let mut clean = 0usize;
    let mut examined = 0usize;
    let units = prefix.get(offset..).unwrap_or(&[]).chunks_exact(variant.unit_bytes()).take(DETECTION_PACKETS);
    for unit in units {
        examined = examined.saturating_add(1);
        let Some(packet) = variant.packet(unit) else { continue };
        let (ok, perfect) = address_quality(packet.byte(ADDRESS_BYTE_0), packet.byte(ADDRESS_BYTE_1));
        usable = usable.saturating_add(usize::from(ok));
        clean = clean.saturating_add(usize::from(perfect));
    }
    (usable, clean, examined)
}

/// Scores sliced records: teletext records whose packet addresses decode,
/// against records with service 0 or teletext.
fn score_sliced(prefix: &[u8], order: ByteOrder) -> (usize, usize, usize) {
    let mut usable = 0usize;
    let mut clean = 0usize;
    let mut examined = 0usize;
    for record in prefix.as_chunks::<SLICED_RECORD_BYTES>().0.iter().take(DETECTION_PACKETS) {
        examined = examined.saturating_add(1);
        let service = order.word(record, 0).unwrap_or(u32::MAX);
        if service != SLICED_TELETEXT_B {
            // Empty records count for the layout without adding evidence.
            if service == 0 {
                usable = usable.saturating_add(1);
            }
            continue;
        }
        let first = record.get(SLICED_HEADER_BYTES).copied().unwrap_or(0);
        let second = record.get(SLICED_HEADER_BYTES.saturating_add(1)).copied().unwrap_or(0);
        let (ok, perfect) = address_quality(first, second);
        usable = usable.saturating_add(usize::from(ok));
        clean = clean.saturating_add(usize::from(perfect));
    }
    (usable, clean, examined)
}

/// Chooses among the headerless binary layouts the one whose packet
/// addresses decode most often (REQ-INP-005 step 2).
fn detect_binary(prefix: &[u8]) -> Option<Detection> {
    let mut best: Option<(Format, usize, (usize, usize, usize))> = None;
    let mut consider = |format: Format, skip: usize, score: (usize, usize, usize)| {
        let (usable, clean, examined) = score;
        if examined == 0 || usable.saturating_mul(ACCEPT_DENOMINATOR) < examined.saturating_mul(ACCEPT_NUMERATOR) {
            return;
        }
        let better = best.is_none_or(|(_, _, (best_usable, best_clean, best_examined))| {
            // Compare usable/examined, then clean/examined, by
            // cross-multiplication.
            let left = usable.saturating_mul(best_examined);
            let right = best_usable.saturating_mul(examined);
            left > right || (left == right && clean.saturating_mul(best_examined) > best_clean.saturating_mul(examined))
        });
        if better {
            best = Some((format, skip, score));
        }
    };
    for variant in T42Variant::ALL {
        for offset in 0..variant.unit_bytes() {
            consider(Format::T42(variant), offset, score_t42(prefix, variant, offset));
        }
    }
    for order in [ByteOrder::Little, ByteOrder::Big] {
        if !Format::Sliced(order).is_compiled() {
            continue;
        }
        let (usable, clean, examined) = score_sliced(prefix, order);
        // Sliced records need at least one teletext record as evidence.
        if clean > 0 || usable > 0 && examined > 0 && order.word(prefix, 0) == Some(SLICED_TELETEXT_B) {
            consider(Format::Sliced(order), 0, (usable, clean, examined));
        }
    }
    best.map(|(format, skip, _)| Detection::Found { format, skip })
}

/// Detects the format of an input from its first bytes (REQ-INP-005).
/// `complete` is true when `prefix` is the whole input. Signatures are
/// tried first; then the headerless binary layouts are scored, which needs
/// [`DETECTION_BYTES`] or the whole input. Only the formats compiled in are
/// recognised ([`Format::is_compiled`]).
#[must_use]
pub fn detect(prefix: &[u8], complete: bool) -> Detection {
    // Every decision waits until the bytes it rests on have all arrived, so
    // that it does not depend on how the input is split (REQ-INP-001).
    let enough = complete || prefix.len() >= DETECTION_BYTES;
    let text = without_bom(prefix);
    let found = |format: Format| Detection::Found { format, skip: prefix.len().saturating_sub(text.len()) };
    if enough || text.contains(&b'\n') {
        if Format::EditTf.is_compiled() && looks_like_edittf(text) {
            return found(Format::EditTf);
        }
        if Format::Tti.is_compiled() && looks_like_tti(text) {
            return found(Format::Tti);
        }
    }
    if enough || prefix.len() > PES_START_CODE.len() {
        if Format::Ep1.is_compiled() && prefix.starts_with(&EP1_SIGNATURE) {
            return Detection::Found { format: Format::Ep1, skip: 0 };
        }
        if Format::Pes.is_compiled()
            && prefix.starts_with(&PES_START_CODE)
            && prefix.get(PES_START_CODE.len()) == Some(&STREAM_ID_PRIVATE_1)
        {
            return Detection::Found { format: Format::Pes, skip: 0 };
        }
    }
    if !enough {
        return Detection::NeedMore;
    }
    for size in TsPacketSize::ALL {
        // The reader finds the first sync byte itself.
        if Format::TransportStream(size).is_compiled() && transport_offset(prefix, size.bytes(), complete).is_some() {
            return Detection::Found { format: Format::TransportStream(size), skip: 0 };
        }
    }
    if Format::ScreenDump.is_compiled() && complete && (prefix.len() == DUMP_BYTES_25 || prefix.len() == DUMP_BYTES_24)
    {
        return Detection::Found { format: Format::ScreenDump, skip: 0 };
    }
    detect_binary(prefix).unwrap_or(Detection::Unknown)
}
