//! Magazine 7: magazine packets M/29.

use neoteletext_core::types::SidePanelFields;

use crate::author::{MagazinePackets, Result, Stream, colour, neutral_fields, neutral_fields_level_35};

use super::{magazine, page, swatches, words};

pub(super) fn magazine_packets(stream: &mut Stream) -> Result<()> {
    let mut data = neutral_fields(0);
    data.side_panels = Some(SidePanelFields { left: true, right: true, status_flag: true, left_columns: 8 });
    data.colour_words = Some(words([
        0x0F0, 0x0E1, 0x0D2, 0x0C3, 0x0B4, 0x0A5, 0x096, 0x087, //
        0x00F, 0x11E, 0x22D, 0x33C, 0x44B, 0x55A, 0x669, 0x778,
    ]));
    data.colours = Some(neoteletext_core::types::DefaultColourFields {
        screen: colour(2, 1),
        row: colour(2, 2),
        black_background_substitution: false,
        remapping: 0,
    });
    let mut level_35 = neutral_fields_level_35(0);
    level_35.colour_words = Some(words([
        0x000, 0xA00, 0x0A0, 0xAA0, 0x00A, 0xA0A, 0x0AA, 0xAAA, //
        0x000, 0x500, 0x050, 0x550, 0x005, 0x505, 0x055, 0x555,
    ]));
    let mut packets = MagazinePackets::new(magazine(7)?);
    packets.data(data).level_35(level_35);
    stream.push(packets.build()?);

    let mut plain = page(0x700, b"M/29/0 AND M/29/4 APPLY")?;
    swatches(&mut plain, 4)?;
    stream.push(plain.build()?);
    let mut page_data = page(0x701, b"X/28/0 BEATS M/29/0")?;
    let mut fields = neutral_fields(0);
    if let Some(colours) = fields.colours.as_mut() {
        colours.screen = colour(0, 4);
    }
    page_data.page_data(fields);
    swatches(&mut page_data, 4)?;
    stream.push(page_data.build()?);
    let mut page_35 = page(0x702, b"X/28/4 BEATS M/29/4 FOR CLUTS 0, 1")?;
    let mut fields = neutral_fields_level_35(0);
    fields.colour_words = Some(words([
        0x000, 0xF44, 0x4F4, 0xFF4, 0x44F, 0xF4F, 0x4FF, 0xCCC, //
        0x000, 0x422, 0x242, 0x442, 0x224, 0x424, 0x244, 0x666,
    ]));
    page_35.level_35_data(fields);
    swatches(&mut page_35, 4)?;
    stream.push(page_35.build()?);
    Ok(())
}
