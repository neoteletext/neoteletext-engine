//! `neottx probe` (DEC-1759 lesson 20): a channel as a receiver meets it,
//! for a developer outside this project. It reads the entry file, makes the
//! push connection, says HELLO, and makes sample selections, reporting how
//! each ended (`INPUT.md` sections 5.1, 6.1 and 7.12).

use neoteletext_app::Failure;
use neoteletext_app::input::Options;

/// `neottx probe`.
#[cfg(feature = "live")]
pub fn probe(options: &Options, selections: &[String]) -> Result<(), Failure> {
    run::probe(options, selections)
}

/// `neottx probe` in a build without live feeds.
#[cfg(not(feature = "live"))]
pub fn probe(_options: &Options, _selections: &[String]) -> Result<(), Failure> {
    Err(neoteletext_app::not_in_build("the channel probe", "live"))
}

#[cfg(feature = "live")]
mod run {
    use super::super::sandbox_closed;
    use neoteletext_app::input::{self, Options};
    use neoteletext_app::pipe::Notice;
    use neoteletext_app::session::Session;
    use neoteletext_app::source::Source;
    use neoteletext_app::{Failure, exit};
    use neoteletext_core::fsm::PageState;
    use neoteletext_core::packet::Packet;
    use neoteletext_core::types::{Instant, PageAddress, SubCode};
    use neoteletext_input::caps::SELECT_CAP;
    use neoteletext_input::control::{FRAGMENT_DATA, from_packet};
    use neoteletext_input::event::{Event, Item, Select, read_message};
    use neoteletext_input::session::{Session as InputSession, State};
    use neoteletext_web::consts::PUSH_SUBPROTOCOL;
    use neoteletext_web::map::Place;
    use std::time::{Duration, Instant as Clock};

    /// How long the entry file and the push connection have to answer:
    /// the longest the fetcher itself waits for a connection and a response.
    const OPENING_WITHIN: Duration = Duration::from_secs(30);
    /// How long a selection has to end: the stated deadline, `INPUT.md`
    /// section 5.4's 5,000 ms, and as long again for a held first selection
    /// and the network. A channel that passes it has broken that promise.
    const ENDING_WITHIN: Duration = Duration::from_secs(10);
    /// How often the probe looks for what arrived: a developer's tool, so
    /// promptness matters less than simplicity.
    const LOOK_EVERY: Duration = Duration::from_millis(5);
    /// A page few channels have, selected to see a selection refused.
    const UNLIKELY_PAGE: &str = "8FC";
    /// DENIED's status for a program that did not answer (`INPUT.md` section
    /// 7.12): the one refusal that is a fault of the channel.
    const STATUS_UNAVAILABLE: u16 = neoteletext_web::records::denied::STATUS_UNAVAILABLE;
    fn fault(text: impl Into<String>) -> Failure {
        (exit::INPUT, text.into())
    }

    /// What the channel has said so far.
    struct Probe {
        session: Session,
        source: Box<dyn Source>,
        input: InputSession,
        start: Clock,
        /// The entry file was read: the channel's initial page.
        initial: Option<u16>,
        /// A selection's ending, while one is awaited.
        denied: Option<(u16, String)>,
        /// The push connection ended for good, and why.
        stopped: Option<String>,
        /// The push connection opened.
        pushing: bool,
    }

    impl Probe {
        fn clock_ms(&self) -> u32 {
            u32::try_from(self.start.elapsed().as_millis()).unwrap_or(u32::MAX)
        }

        /// Takes what arrived since the last look.
        fn look(&mut self) -> Result<(), Failure> {
            let now = Instant(u64::try_from(self.start.elapsed().as_micros()).unwrap_or(u64::MAX));
            let alive = self.source.poll(&mut self.session, now);
            for notice in self.source.notices() {
                self.take(notice)?;
            }
            for packet in self.session.take_kept() {
                self.record(&packet);
            }
            self.input.tick(self.clock_ms());
            if alive { Ok(()) } else { Err(fault("the fetcher process ended")) }
        }

        fn take(&mut self, notice: Notice) -> Result<(), Failure> {
            match notice {
                Notice::ServiceName(name) => println!("entry file  read: {name:?}"),
                Notice::InitialPage(mpp) => {
                    println!("initial     page {mpp:03X}");
                    self.initial = Some(mpp);
                }
                Notice::Input(open) if open != self.pushing => {
                    self.pushing = open;
                    if open {
                        println!("handshake   accepted: {PUSH_SUBPROTOCOL}");
                        let mut out = [0_u8; neoteletext_input::MAX_MESSAGE];
                        let length = self
                            .input
                            .open(self.clock_ms(), None, &mut out)
                            .map_err(|error| fault(format!("{error:?}")))?;
                        self.source.send_input(out.get(..length).unwrap_or(&[]));
                    } else {
                        self.input.close();
                    }
                }
                Notice::Denied(status, place) => self.denied = Some((status, place)),
                Notice::PushStopped(why) => self.stopped = Some(why),
                Notice::Message(text) => println!("said        {text}"),
                _ => {}
            }
            Ok(())
        }

        /// A channel's record: WELCOME goes to the input session.
        fn record(&mut self, packet: &Packet) {
            let mut buffer = [0; FRAGMENT_DATA];
            if let Some(Ok(control)) = from_packet(packet, &mut buffer) {
                let _ = self.input.control(&control);
            }
        }

        /// Looks until `done` says so, or `within` passes.
        fn until(&mut self, within: Duration, done: impl Fn(&Self) -> bool) -> Result<bool, Failure> {
            let from = Clock::now();
            while from.elapsed() < within {
                self.look()?;
                if let Some(why) = &self.stopped {
                    return Err(fault(why.clone()));
                }
                if done(self) {
                    return Ok(true);
                }
                std::thread::sleep(LOOK_EVERY);
            }
            Ok(false)
        }

        /// Makes the selection `place` says, and reports how it ended.
        fn select(&mut self, text: &str) -> Result<(), Failure> {
            let Some(Place::Page { page, subcode, parameters }) = Place::read(text) else {
                return Err((exit::USAGE, format!("{text:?} is not a page or a sub-page")));
            };
            self.denied = None;
            let shown = subcode.unwrap_or(SubCode::ANY);
            let began = Clock::now();
            self.source.request(page, shown);
            if self.input.welcomed() {
                let select =
                    Select::new(page.mpp(), shown.value()).ok_or_else(|| fault("a page no SELECT can name"))?;
                let item = Item::Event(Event::Select(select, parameters.unwrap_or_default()));
                let mut out = [0_u8; neoteletext_input::MAX_MESSAGE];
                let length =
                    self.input.send(&item, self.clock_ms(), &mut out).map_err(|error| fault(format!("{error:?}")))?;
                let message = out.get(..length).unwrap_or(&[]);
                read_message(message, &mut |_| {}).map_err(|error| fault(format!("{error:?}")))?;
                self.source.send_input(message);
            }
            let ended = self.until(ENDING_WITHIN, |probe| probe.denied.is_some() || probe.shows(page, shown))?;
            let elapsed = began.elapsed().as_millis();
            if !ended {
                return Err(fault(format!(
                    "selection {text}: no page and no DENIED within {} s",
                    ENDING_WITHIN.as_secs()
                )));
            }
            match self.denied.take() {
                Some((status, _)) if status == STATUS_UNAVAILABLE => {
                    Err(fault(format!("selection {text}: DENIED {status}: the channel's program did not answer")))
                }
                Some((status, place)) => {
                    println!("selection   {text}: DENIED {status} for {place:?} in {elapsed} ms");
                    Ok(())
                }
                None => {
                    println!("selection   {text}: a page in {elapsed} ms");
                    Ok(())
                }
            }
        }

        fn shows(&self, page: PageAddress, sub: SubCode) -> bool {
            self.session.page(page, sub).is_some_and(|stored| stored.state == PageState::Complete)
        }
    }

    pub fn probe(options: &Options, selections: &[String]) -> Result<(), Failure> {
        if !input::is_live(&options.input) {
            return Err((exit::USAGE, format!("{:?} is not the address of a channel", options.input)));
        }
        let opening = input::opening(&options.input).map_err(|message| (exit::USAGE, message))?;
        let (mut session, source) = input::open_live(options)?;
        session.keep(neoteletext_input::is_input_packet);
        sandbox_closed(options.no_sandbox)?;
        println!("channel     {}", options.input);
        let mut probe = Probe {
            session,
            source,
            input: InputSession::new(SELECT_CAP),
            start: Clock::now(),
            initial: None,
            denied: None,
            stopped: None,
            pushing: false,
        };
        if !probe.until(OPENING_WITHIN, |probe| probe.initial.is_some())? {
            return Err(fault("the entry file gave no initial page"));
        }
        // A live channel's connection opens straight after its entry file,
        // and a static channel's never does: the selection then goes as a fetch.
        let welcomed = |probe: &Probe| probe.input.welcomed() || probe.input.state() == State::Refused;
        let live = probe.until(ENDING_WITHIN, |probe| probe.pushing)?;
        if live {
            if !probe.until(ENDING_WITHIN, welcomed)? || !probe.input.welcomed() {
                return Err(fault("the channel did not answer HELLO with WELCOME"));
            }
            println!("protocol    input protocol {}, WELCOME received", neoteletext_input::VERSION);
        } else {
            println!("protocol    no push connection: a static channel, read by its files");
        }
        let first = opening.page.map_or_else(
            || format!("{:03X}", probe.initial.unwrap_or_default()),
            |(page, sub)| {
                let place = if sub.is_any() {
                    format!("{:03X}", page.mpp())
                } else {
                    format!("{:03X}/{:04X}", page.mpp(), sub.value())
                };
                if opening.parameters.is_empty() { place } else { format!("{place}?{}", opening.parameters) }
            },
        );
        for place in std::iter::once(first.as_str())
            .chain(selections.iter().map(String::as_str))
            .chain(live.then_some(UNLIKELY_PAGE))
        {
            probe.select(place)?;
        }
        println!("result      no fault");
        Ok(())
    }
}
