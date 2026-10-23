//! `neottx fetch`, the fetcher process of live mode (REQ-SEC-018), or its
//! refusal in a build without live feeds.

use neoteletext_app::Failure;
use std::net::IpAddr;

/// `neottx fetch`, the fetcher process of live mode (REQ-SEC-018), which
/// `neottx` starts by running itself again.
#[cfg(feature = "live")]
pub fn fetch(address: &str, no_sandbox: bool, allow_local: &[IpAddr], proxy: Option<&str>) -> Result<(), Failure> {
    neoteletext_app::fetch::run(address, no_sandbox, allow_local, proxy)
}

/// `neottx fetch` in a build without live feeds.
#[cfg(not(feature = "live"))]
pub fn fetch(_address: &str, _no_sandbox: bool, _allow_local: &[IpAddr], _proxy: Option<&str>) -> Result<(), Failure> {
    Err(neoteletext_app::not_in_build("the fetcher process of live mode", "live"))
}
