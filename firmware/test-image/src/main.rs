//! The linked microcontroller test image (REQ-GEN-009, REQ-GEN-019).
//!
//! Every entry point a port calls is rooted in a `#[used]` function pointer.
//! The functions take their inputs as arguments, so nothing is evaluated at
//! compile time, and link-time optimisation keeps exactly the code a port
//! using the same features would link: no more, because nothing else is
//! reachable, and no less, because the pointers are kept. The image is
//! never run; `cargo xtask size` measures it and checks its symbols.
//!
//! The image's features choose the build being measured (decision E.61).
#![no_std]
#![no_main]
// The image is never run, so it has no entry point, and the linker says so.
#![allow(linker_messages)]

use core::hint::black_box;
use neoteletext_core::acq::{Decoder, StoredPage};
use neoteletext_core::compose::compose_level;
use neoteletext_core::config::Config;
use neoteletext_core::level1::{DisplayOptions, Grid, build_grid};
use neoteletext_core::packet::{Packet, PacketMeta, ServicePacket};
use neoteletext_core::service;
use neoteletext_core::types::{Instant, PageAddress, SubCode};

/// The image's own panic handler: the only one REQ-GEN-019 allows.
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// Acquisition: a decoder over the caller's store, fed packets and time.
fn receive(pages: &mut [StoredPage], config: Config, packet: &Packet, meta: &PacketMeta, now: Instant) {
    let mut decoder = Decoder::new(pages, config);
    decoder.push(packet, meta, now, &mut |event| {
        black_box(event);
    });
    decoder.tick(now, &mut |event| {
        black_box(event);
    });
    black_box(decoder.statistics());
}

/// Composing a page at the configured level.
fn compose(decoder: &Decoder<'_>, address: PageAddress, config: &Config, grid: &mut Grid) {
    let page = decoder.store().get(address, SubCode::ANY);
    let magazine = decoder.magazine_data(address.magazine);
    let options = DisplayOptions::default();
    build_grid(grid, page, &magazine, config, &options);
    if let Some(page) = page {
        black_box(compose_level(grid, page, &magazine, &decoder.enhancement_lookup(), config, &options));
    }
}

/// Broadcast service data, packet 8/30.
fn service_data(packet: &ServicePacket) {
    let info = service::interpret(packet);
    black_box(service::initial_page(Some(&info)));
}

#[used]
static RECEIVE: fn(&mut [StoredPage], Config, &Packet, &PacketMeta, Instant) = receive;
#[used]
static COMPOSE: fn(&Decoder<'_>, PageAddress, &Config, &mut Grid) = compose;
#[used]
static SERVICE: fn(&ServicePacket) = service_data;

/// Levels 2.5 and 3.5: a decoder that also stores enhancement pages, and
/// records the processor's report.
#[cfg(feature = "level25")]
mod level25 {
    use super::{Config, Decoder, Instant, Packet, PacketMeta, StoredPage, black_box};
    use neoteletext_core::acq::{EnhancementPacket, EnhancementReport, EnhancementStore, EnhancementSubPage};

    fn receive(
        pages: &mut [StoredPage],
        sub_pages: &mut [EnhancementSubPage],
        packets: &mut [EnhancementPacket],
        config: Config,
        packet: &Packet,
        now: Instant,
        report: &EnhancementReport,
    ) {
        let mut decoder = Decoder::with_enhancement_store(pages, EnhancementStore::new(sub_pages, packets), config);
        decoder.push(packet, &PacketMeta::default(), now, &mut |event| {
            black_box(event);
        });
        black_box(decoder.record_enhancement(report.build, &report.faults));
    }

    #[used]
    static RECEIVE: fn(
        &mut [StoredPage],
        &mut [EnhancementSubPage],
        &mut [EnhancementPacket],
        Config,
        &Packet,
        Instant,
        &EnhancementReport,
    ) = receive;
}

/// The row renderer, with whatever glyphs the port supplies.
#[cfg(feature = "render")]
mod render {
    use super::{Grid, black_box};
    use neoteletext_core::glyphs::GlyphSet;
    use neoteletext_core::render::{Field, IndexBuffer, RenderOptions, render_row};

    fn render(grid: &Grid, row: usize, glyphs: &GlyphSet, options: RenderOptions, buffer: &mut IndexBuffer<'_>) {
        let _ = black_box(render_row(grid, row, Field::Both, glyphs, options, buffer));
    }

    #[used]
    static RENDER: fn(&Grid, usize, &GlyphSet, RenderOptions, &mut IndexBuffer<'_>) = render;
}

/// Bedstead, the one bundled font, as a port that uses it links it.
#[cfg(feature = "bedstead")]
mod bedstead {
    use super::black_box;
    use neoteletext_core::glyphs::GlyphSet;

    fn glyphs() {
        black_box(GlyphSet::bedstead());
    }

    #[used]
    static GLYPHS: fn() = glyphs;
}

/// The viewer: keys, decoder events, the status area and composing.
#[cfg(feature = "viewer")]
mod viewer {
    use super::{Grid, StoredPage, black_box};
    use neoteletext_core::viewer::{ComposeContext, Key, KeyContext, Viewer, ViewerEvent};

    fn view(
        viewer: &mut Viewer,
        key: Key,
        event: ViewerEvent,
        keys: &KeyContext<'_>,
        compose: &ComposeContext<'_>,
        page: Option<&StoredPage>,
        grid: &mut Grid,
    ) {
        black_box(viewer.key(key, keys));
        black_box(viewer.event(event));
        black_box(viewer.compose(grid, page, compose));
    }

    #[used]
    static VIEW: fn(
        &mut Viewer,
        Key,
        ViewerEvent,
        &KeyContext<'_>,
        &ComposeContext<'_>,
        Option<&StoredPage>,
        &mut Grid,
    ) = view;
}

/// Reading input: format detection and whichever readers are compiled in.
#[cfg(feature = "formats")]
mod formats {
    use super::black_box;
    use neoteletext_formats::detect::{AutoConfig, AutoReader};
    use neoteletext_formats::item::Reader;

    fn read(config: AutoConfig, bytes: &[u8]) {
        let mut reader = AutoReader::new(config);
        reader.feed(bytes, &mut |item| {
            black_box(item);
        });
        reader.finish(&mut |item| {
            black_box(item);
        });
    }

    #[used]
    static READ: fn(AutoConfig, &[u8]) = read;
}

/// The web user agent, with a port's random source.
#[cfg(feature = "web")]
mod web {
    use super::{Instant, black_box};
    use neoteletext_web::agent::{Input, UserAgent};
    use neoteletext_web::port::RandomSource;

    /// A stand-in for the port's random source.
    pub struct Random;

    impl RandomSource for Random {
        fn fill(&mut self, bytes: &mut [u8]) {
            for byte in bytes {
                *byte = black_box(0);
            }
        }
    }

    /// Slots and cookie bytes as a small port might choose.
    type Agent<'m> = UserAgent<'m, Random, 4, 4096>;

    fn handle(agent: &mut Agent<'_>, now: Instant, input: Input<'_>) {
        agent.handle(now, input, &mut |output| {
            black_box(output);
        });
    }

    #[used]
    static HANDLE: fn(&mut Agent<'_>, Instant, Input<'_>) = handle;
}

/// The Opus decoder (decision E.123, REQ-AUD-024): a packet decoded, or a loss concealed, into a
/// frame.
#[cfg(feature = "opus")]
mod opus {
    use super::black_box;
    use neoteletext_opus::{Decoder, FRAME_SAMPLES};

    /// A decoder made in the caller's place, as a port that keeps it in a static would.
    fn new(place: &mut Decoder) {
        *place = Decoder::new();
    }

    fn decode(decoder: &mut Decoder, packet: &[u8], pcm: &mut [i16; FRAME_SAMPLES]) {
        black_box(decoder.decode(packet, pcm).is_ok());
    }

    fn conceal(decoder: &mut Decoder, pcm: &mut [i16; FRAME_SAMPLES]) {
        decoder.conceal(pcm);
    }

    fn reset(decoder: &mut Decoder) {
        decoder.reset();
    }

    fn final_range(decoder: &Decoder) -> u32 {
        decoder.final_range()
    }

    // Every entry point, so that a panic path in any of them reaches the symbol check.
    #[used]
    static NEW: fn(&mut Decoder) = new;
    #[used]
    static DECODE: fn(&mut Decoder, &[u8], &mut [i16; FRAME_SAMPLES]) = decode;
    #[used]
    static CONCEAL: fn(&mut Decoder, &mut [i16; FRAME_SAMPLES]) = conceal;
    #[used]
    static RESET: fn(&mut Decoder) = reset;
    #[used]
    static FINAL_RANGE: fn(&Decoder) -> u32 = final_range;
}

/// REQ-GEN-019: one floating-point operation, which the symbol check must
/// find on targets without a floating-point unit.
#[cfg(feature = "float-probe")]
mod float_probe {
    use super::black_box;

    fn scale(value: f32) -> f32 {
        black_box(value) * black_box(1.5)
    }

    #[used]
    static SCALE: fn(f32) -> f32 = scale;
}
