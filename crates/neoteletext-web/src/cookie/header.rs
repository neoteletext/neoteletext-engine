//! The `Cookie` header of a request: which cookies match and in what order
//! (RFC 6265bis section 5.8.3, REQ-WEB-006), and the memory that a host
//! was reached securely (REQ-SEC-007).

use super::store::path_matches;
use super::{CookieJar, SameSite, Site};
use crate::bytes::{Overflow, Writer};
use neoteletext_core::policy;

impl<const SLOTS: usize, const BYTES: usize> CookieJar<SLOTS, BYTES> {
    /// Writes the `Cookie` header value for a request (RFC 6265bis section
    /// 5.8.3): `name=value` pairs joined by `; `, longest path first, then
    /// oldest first. Returns the number of cookies written, and whether
    /// some did not fit in `out`.
    pub fn header_for(
        &mut self,
        host: &str,
        path: &str,
        secure: bool,
        site: Site,
        now: u64,
        out: &mut Writer<'_>,
    ) -> (usize, bool) {
        let mut order = [0usize; SLOTS];
        let mut count = 0usize;
        for (index, slot) in self.records.iter().enumerate() {
            let Some(record) = slot else { continue };
            let parts = self.parts(record);
            let wanted = parts.host == host.as_bytes()
                && path_matches(path.as_bytes(), parts.path)
                && policy::may_send_cookie(record.secure, secure)
                && !record.expired(now)
                && match site {
                    Site::SameService => true,
                    Site::CrossServiceNavigation => record.same_site != SameSite::Strict,
                    Site::CrossOriginForm => record.same_site == SameSite::None && record.secure,
                };
            if wanted && let Some(entry) = order.get_mut(count) {
                *entry = index;
                count = count.saturating_add(1);
            }
        }
        let selected = order.get_mut(..count).unwrap_or(&mut []);
        // Insertion sort: at most SLOTS entries.
        for i in 1..selected.len() {
            let mut j = i;
            while j > 0 {
                let (Some(&a), Some(&b)) = (selected.get(j.saturating_sub(1)), selected.get(j)) else { break };
                if self.sort_key(b) < self.sort_key(a) {
                    selected.swap(j.saturating_sub(1), j);
                    j = j.saturating_sub(1);
                } else {
                    break;
                }
            }
        }
        let mut written = 0usize;
        let mut truncated = false;
        let tick = self.next_tick();
        for &index in selected.iter() {
            let Some(record) = self.records.get(index).copied().flatten() else { continue };
            let mark = out.len();
            let parts = self.parts(&record);
            let result = (|| -> Result<(), Overflow> {
                if written > 0 {
                    out.put(b"; ")?;
                }
                if !parts.name.is_empty() {
                    out.put(parts.name)?;
                    out.byte(b'=')?;
                }
                out.put(parts.value)
            })();
            if result.is_err() {
                out.truncate(mark);
                truncated = true;
                continue;
            }
            written = written.saturating_add(1);
            if let Some(Some(stored)) = self.records.get_mut(index) {
                stored.last_access = tick;
                stored.used_at = now;
            }
        }
        (written, truncated)
    }

    fn sort_key(&self, index: usize) -> (core::cmp::Reverse<u16>, u64) {
        self.records
            .get(index)
            .copied()
            .flatten()
            .map_or((core::cmp::Reverse(0), u64::MAX), |record| (core::cmp::Reverse(record.path_len), record.created))
    }

    /// True when a cookie for `host` was set over an encrypted connection
    /// and is still live: the host has been reached over HTTPS "for as long
    /// as its cookies last" (REQ-SEC-007).
    #[must_use]
    pub fn reached_securely(&self, host: &str, now: u64) -> bool {
        self.records
            .iter()
            .flatten()
            .any(|record| record.via_secure && !record.expired(now) && self.parts(record).host == host.as_bytes())
    }
}
