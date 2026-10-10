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
use tellytxt_core::acq::{Decoder, StoredPage};
use tellytxt_core::compose::compose_level;
use tellytxt_core::config::Config;
use tellytxt_core::level1::{DisplayOptions, Grid, build_grid};
use tellytxt_core::packet::{Packet, PacketMeta, ServicePacket};
use tellytxt_core::service;
use tellytxt_core::types::{Instant, PageAddress, SubCode};

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
    use tellytxt_core::acq::{EnhancementPacket, EnhancementReport, EnhancementStore, EnhancementSubPage};

    fn receive(
        pages: &mut [StoredPage],
        sub_pages: &mut [EnhancementSubPage],
        packets: &mut [EnhancementPacket],
        config: Config,
        packet: &Packet,
        now: Instant,
        report: &EnhancementReport,
    ) {
        let mut decoder =
            Decoder::with_enhancement_store(pages, EnhancementStore::new(sub_pages, packets), config);
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
    use tellytxt_core::glyphs::GlyphSet;
    use tellytxt_core::render::{Field, IndexBuffer, RenderOptions, render_row};

    fn render(grid: &Grid, row: usize, glyphs: &GlyphSet, options: RenderOptions, buffer: &mut IndexBuffer<'_>) {
        let _ = black_box(render_row(grid, row, Field::Both, glyphs, options, buffer));
    }

    #[used]
    static RENDER: fn(&Grid, usize, &GlyphSet, RenderOptions, &mut IndexBuffer<'_>) = render;
}

/// The temporary Bedstead glyphs, as a port that uses them links them.
#[cfg(feature = "bedstead")]
mod bedstead {
    use super::black_box;
    use tellytxt_core::glyphs::GlyphSet;

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
    use tellytxt_core::viewer::{ComposeContext, Key, KeyContext, Viewer, ViewerEvent};

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

/// Forms: building the model, keys, validation, the submitted pairs and
/// drawing.
#[cfg(feature = "forms")]
mod forms {
    use super::{Grid, Instant, black_box};
    use tellytxt_core::charset::G0;
    use tellytxt_core::forms::{FormKey, FormModel, FormSpec};

    fn form(spec: FormSpec, g0: G0, key: FormKey, now: Instant, grid: &mut Grid) {
        if let Ok(mut model) = FormModel::new(spec, g0) {
            black_box(model.key(key, now));
            let _ = black_box(model.validate());
            model.pairs(&mut |name, value| {
                black_box((name, value));
            });
            model.draw(grid);
        }
    }

    #[used]
    static FORM: fn(FormSpec, G0, FormKey, Instant, &mut Grid) = form;
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
