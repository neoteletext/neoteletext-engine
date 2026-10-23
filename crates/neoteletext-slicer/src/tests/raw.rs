//! REQ-INP-011: raw sampled video files.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::similar_names
)]

use super::*;

/// Builds `frames` bt8x8 frames; returns the file and, per line index in
/// the file, the packet it carries.
fn bt8x8_file(frames: usize) -> (Vec<u8>, Vec<Option<Packet>>) {
    let geometry = RawGeometry::preset(Preset::Bt8x8);
    let mut generator = Generator::new(21);
    // The capture starts about 7 µs after the synchronising edge, so the
    // run-in begins some 110 samples in and the next line's sync is at the
    // end of the 2048 samples.
    let params = SignalParams { start_ns: 7_000.0, ..SignalParams::nominal(geometry.sample_rate_hz as f32) };
    let mut file = Vec::new();
    let mut packets = Vec::new();
    for frame in 0..frames {
        for line in 0..geometry.lines_per_frame() {
            // Every third line is empty.
            let packet = (line % 3 != 2).then(|| random_packet(generator.rng()));
            let bytes = packet.as_ref().map_or([0u8; PACKET_BYTES_WITH_RUN_IN], line_bytes);
            file.extend(render(&mut generator, &params, &bytes, geometry.samples_per_line));
            packets.push(packet);
        }
        // The driver's frame sequence number in the last four bytes.
        let end = file.len();
        file[end - 4..].copy_from_slice(&(frame as u32 * 0x0101_0101 + 0x8000_00FF).to_le_bytes());
    }
    (file, packets)
}

#[test]
fn raw_bt8x8_file_in_odd_chunks() {
    let (file, packets) = bt8x8_file(3);
    let geometry = RawGeometry::preset(Preset::Bt8x8);
    let mut reader = RawReader::new(geometry, Config::new()).unwrap();
    let mut received: Vec<(Packet, PacketMeta)> = Vec::new();
    let sizes = [1usize, 7, 333, 4097, 2048, 65_539, 13];
    let mut offset = 0;
    let mut size_index = 0;
    while offset < file.len() {
        let size = sizes[size_index % sizes.len()].min(file.len() - offset);
        reader.feed(&file[offset..offset + size], |packet, meta| received.push((packet, meta)));
        offset += size;
        size_index += 1;
    }
    assert_eq!(reader.frames(), 3);
    let expected: Vec<(usize, Packet)> =
        packets.iter().enumerate().filter_map(|(index, packet)| packet.map(|p| (index, p))).collect();
    assert_eq!(received.len(), expected.len());
    for ((packet, meta), (index, want)) in received.iter().zip(&expected) {
        assert_eq!(packet, want, "line {index}");
        let in_frame = index % geometry.lines_per_frame();
        let field = in_frame / geometry.lines_per_field;
        let line = [7u16, 320][field] + (in_frame % geometry.lines_per_field) as u16;
        assert_eq!(meta.line, Some(line));
        assert_eq!(meta.second_field, Some(field == 1));
        let fields = (index / geometry.lines_per_frame() * 2 + field) as u64;
        assert_eq!(meta.timestamp_us, Some(fields * 20_000));
        assert!(meta.quality.is_some());
    }
}

#[test]
fn raw_reader_other_presets() {
    // One frame of each 16-line preset with a packet on every line.
    for preset in [Preset::Cx88, Preset::Saa7131, Preset::TbcVbi] {
        let geometry = RawGeometry::preset(preset);
        let mut generator = Generator::new(22);
        let rate = geometry.sample_rate_hz;
        // Start each window so that it holds the whole packet; the saa7131
        // window is only 53.3 µs, so it must start just before the data.
        let start = match preset {
            Preset::Saa7131 => 9_500.0,
            Preset::Cx88 => 6_000.0,
            Preset::Bt8x8 | Preset::Tbc | Preset::TbcVbi => 4_000.0,
        };
        let params = SignalParams { start_ns: start, ..SignalParams::nominal(rate as f32) };
        let mut file = Vec::new();
        let mut used = 0;
        for line in 0..geometry.lines_per_frame() {
            let packet = random_packet(generator.rng());
            let mut values = vec![0.0_f32; geometry.samples_per_line];
            generator.line(&params, &line_bytes(&packet), &mut values);
            let mut bytes = vec![0u8; geometry.line_bytes()];
            match geometry.format {
                SampleFormat::U8 => quantise_u8(&values, &mut bytes),
                SampleFormat::U16Le => quantise_u16le(&values, SAMPLE_16_PER_8 as f32, &mut bytes),
            }
            file.extend(bytes);
            let in_field = line % geometry.lines_per_field;
            if in_field >= geometry.first_used && in_field <= geometry.last_used {
                used += 1;
            }
        }
        let mut reader = RawReader::new(geometry, Config::new()).unwrap();
        let mut count = 0;
        for chunk in file.chunks(1001) {
            reader.feed(chunk, |_, meta| {
                assert_eq!(meta.line, None);
                count += 1;
            });
        }
        assert_eq!(count, used, "{preset:?}");
    }
}

#[test]
fn raw_geometry_presets_and_validation() {
    let bt = RawGeometry::preset(Preset::Bt8x8);
    assert_eq!(bt, RawGeometry::default());
    assert_eq!((bt.sample_rate_hz, bt.samples_per_line, bt.lines_per_field), (35_468_950, 2048, 16));
    let tbc = RawGeometry::preset(Preset::Tbc);
    assert_eq!((tbc.sample_rate_hz, tbc.samples_per_line, tbc.format), (17_730_000, 1135, SampleFormat::U16Le));
    assert_eq!((tbc.lines_per_field, tbc.first_used, tbc.last_used), (313, 6, 21));
    let cx = RawGeometry::preset(Preset::Cx88);
    assert_eq!((cx.lines_per_field, cx.first_used, cx.last_used), (18, 1, 16));
    for preset in [Preset::Bt8x8, Preset::Cx88, Preset::Saa7131, Preset::Tbc, Preset::TbcVbi] {
        assert_eq!(RawGeometry::preset(preset).validate(), Ok(()));
    }
    let bad = |change: fn(&mut RawGeometry)| {
        let mut geometry = bt;
        change(&mut geometry);
        RawReader::new(geometry, Config::new()).err()
    };
    assert_eq!(bad(|g| g.samples_per_line = 0), Some(GeometryError::LineLength));
    assert_eq!(bad(|g| g.samples_per_line = 1 << 20), Some(GeometryError::LineLength));
    assert_eq!(bad(|g| g.lines_per_field = 0), Some(GeometryError::NoLines));
    assert_eq!(bad(|g| g.last_used = 16), Some(GeometryError::UsedLines));
    assert_eq!(
        bad(|g| {
            g.first_used = 10;
            g.last_used = 5;
        }),
        Some(GeometryError::UsedLines)
    );
    assert_eq!(bad(|g| g.frame_trailer_bytes = 2048), Some(GeometryError::Trailer));
}
