//! The live feed's loop (REQ-INP-022): zxnet over the std transport, or any
//! transport in the tests.

use super::decoder_link::{page_request, send};
use super::transport::{READ_CHUNK, StdTransport, SystemRandom, web_address};
use super::wakes::{Wake, Wakes};
use super::{clock, neottx};
use crate::pipe::{Frame, Notice};
use neoteletext_core::types::Instant;
use neoteletext_web::port::{ConnectionId, Pump, Transport};
use neoteletext_web::proxy::Proxy;
use neoteletext_web::zxnet::{Channel, LiveEvent, LiveInput, LiveOutput, LiveStorage, ZxnetFeed};
use std::cell::Cell;
use std::net::IpAddr;
use std::sync::mpsc::Sender;
use std::time::Instant as StdInstant;

pub(super) fn run_zxnet(
    channel: &Channel,
    wakes: &mut Wakes,
    wake: Sender<Wake>,
    allow_local: &[IpAddr],
    proxy: Option<Proxy>,
) {
    let proxied = proxy.is_some();
    let transport = StdTransport::new(allow_local, wake, proxy);
    live_loop(channel, wakes, transport, allow_local, proxied, &mut |frame| send(frame));
}

/// The live feed's loop over any transport, sending what it receives
/// through `emit` until that fails or the decoder process goes.
pub(super) fn live_loop<T: Transport>(
    channel: &Channel,
    wakes: &mut Wakes,
    transport: T,
    allow_local: &[IpAddr],
    proxied: bool,
    emit: &mut dyn FnMut(&Frame) -> std::io::Result<()>,
) {
    let mut storage = Box::new(LiveStorage::default());
    let mut feed = new_feed(channel, &mut storage, allow_local, proxied);
    let mut pump = Pump::new(transport);
    let start = StdInstant::now();
    let mut buffer = vec![0u8; READ_CHUNK];
    let broken = Cell::new(false);
    // The feed says, after every input, when it next needs a tick, and
    // says nothing once it has stopped: the latest it said is the deadline.
    let deadline: Cell<Option<Instant>> = Cell::new(None);
    let mut sink = |output: LiveOutput<'_>, pump: &mut Pump<T>| {
        if let Some(net) = output.net() {
            pump.execute(net);
            return;
        }
        if forward(output, &deadline, emit).is_err() {
            broken.set(true);
        }
    };
    let mut step = |input: LiveInput<'_>, pump: &mut Pump<T>| {
        deadline.set(None);
        feed.handle(clock(start), input, &mut |output| sink(output, pump));
        deadline.get()
    };
    let mut next_tick = step(LiveInput::Start, &mut pump);
    while !broken.get() && !wakes.ended() {
        wakes.turn();
        let mut busy = false;
        if let Some(input) = pump.poll(ConnectionId::Live, &mut buffer) {
            busy = true;
            next_tick = step(LiveInput::Net(input), &mut pump);
        }
        while let Some(frame) = wakes.next_request() {
            busy = true;
            if let Frame::Request(mpp, sub) = frame
                && let Some((page, subcode)) = page_request(mpp, sub)
            {
                next_tick = step(LiveInput::Request { page, subcode }, &mut pump);
            }
        }
        if next_tick.is_some_and(|at| clock(start) >= at) {
            // What a tick asks for, a reconnection above all, is taken up
            // on the next turn, not after another wait.
            busy = true;
            next_tick = step(LiveInput::Tick, &mut pump);
        }
        if !busy {
            wakes.wait(clock(start), next_tick);
        }
    }
}

/// The live feed for `channel`, named, told whether it goes through a
/// proxy, and given the local address it may reach.
fn new_feed<'a>(
    channel: &Channel,
    storage: &'a mut LiveStorage,
    allow_local: &[IpAddr],
    proxied: bool,
) -> ZxnetFeed<'a, SystemRandom> {
    let mut feed = ZxnetFeed::new(*channel, storage.buffers(), SystemRandom);
    feed.name_product(neottx().as_ref());
    if proxied {
        feed.through_proxy();
    }
    if let Some(&address) = allow_local.first() {
        feed.allow_local(web_address(address));
    }
    feed
}

/// Acts on one of the feed's outputs that is not for the network: a timer
/// sets the deadline, and packets and the events the decoder shows go to
/// `emit`.
fn forward(
    output: LiveOutput<'_>,
    deadline: &Cell<Option<Instant>>,
    emit: &mut dyn FnMut(&Frame) -> std::io::Result<()>,
) -> std::io::Result<()> {
    match output {
        LiveOutput::Timer(at) => {
            deadline.set(Some(at));
            Ok(())
        }
        LiveOutput::Packet(packet) => emit(&Frame::Packet(packet.bytes)),
        LiveOutput::Event(LiveEvent::InitialPage { page, .. }) => emit(&Frame::Notice(Notice::InitialPage(page.mpp()))),
        LiveOutput::Event(LiveEvent::GaveUp) => {
            emit(&Frame::Notice(Notice::Message("live feed: gave up reconnecting".into())))
        }
        LiveOutput::Event(LiveEvent::WrongName) => {
            emit(&Frame::Notice(Notice::Message("live feed: wrong service name".into())))
        }
        _ => Ok(()),
    }
}
