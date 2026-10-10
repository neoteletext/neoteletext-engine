//! Raw sampled video files (REQ-INP-011).
//!
//! A headerless file of raw sampled lines from the blanking interval: each
//! frame is two fields, each field a fixed number of lines, each line a
//! fixed number of samples. [`RawReader`] is fed the file in chunks of any
//! size, splits it into lines and passes each used line to the [`Slicer`].
//!
//! The presets follow the capture devices that the vhs-teletext tool reads.
//! The "lines used" of a preset are zero-based indices within a field, as
//! that tool counts them.

use tellytxt_core::consts::FIELD_PERIOD_US;
use tellytxt_core::packet::{Packet, PacketMeta};
use tellytxt_core::trace::TraceEntry;

use crate::consts::MAX_LINE_SAMPLES;
use crate::slicer::{Config, Line, SampleFormat, Slicer};

/// The most bytes of one line: [`MAX_LINE_SAMPLES`] 16-bit samples.
pub const MAX_LINE_BYTES: usize = MAX_LINE_SAMPLES * 2;
/// Fields per frame.
const FIELDS_PER_FRAME: usize = 2;
/// The PAL sampling rate of the bt8x8 and cx88 drivers: 8 × the PAL colour
/// subcarrier, hertz (REQ-INP-011 table).
const RATE_8_FSC_PAL: u32 = 35_468_950;
/// The saa7131 sampling rate, hertz (REQ-INP-011 table).
const RATE_SAA7131: u32 = 27_000_000;
/// The ld-decode time-base-corrected rate, hertz (REQ-INP-011 table).
const RATE_TBC: u32 = 17_730_000;
/// Samples per line of the bt8x8 and cx88 captures (REQ-INP-011 table).
const SAMPLES_8_FSC: usize = 2048;
/// Samples per line of the saa7131 capture (REQ-INP-011 table).
const SAMPLES_SAA7131: usize = 1440;
/// Samples per line of the tbc captures (REQ-INP-011 table).
const SAMPLES_TBC: usize = 1135;
/// Lines per field of the bt8x8, saa7131 and tbc-vbi captures.
const LINES_16: usize = 16;
/// Lines per field of the cx88 capture.
const LINES_CX88: usize = 18;
/// Lines per field of a full tbc field.
const LINES_TBC: usize = 313;
/// Lines used of the cx88 capture: indices 1 to 16.
const CX88_USED: (usize, usize) = (1, 16);
/// Lines used of a full tbc field: indices 6 to 21.
const TBC_USED: (usize, usize) = (6, 21);
/// Television line numbers of the first bt8x8 line of each field: lines 7
/// to 22 and 320 to 335 (REQ-INP-011 table).
const BT8X8_FIRST_LINES: [u16; 2] = [7, 320];
/// Bytes of the frame sequence number the Linux bt8x8 driver may write at
/// the end of each frame (REQ-INP-011).
const BT8X8_SEQUENCE_BYTES: usize = 4;

/// The capture presets of REQ-INP-011.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preset {
    /// Brooktree `Bt848` and `Bt878` (the default).
    Bt8x8,
    /// Conexant `CX2388x`.
    Cx88,
    /// Philips SAA7131.
    Saa7131,
    /// ld-decode time-base-corrected output, whole fields.
    Tbc,
    /// ld-decode time-base-corrected output, blanking lines only.
    TbcVbi,
}

/// The layout of a raw sampled video file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RawGeometry {
    /// Sample rate in hertz.
    pub sample_rate_hz: u32,
    /// Samples per line.
    pub samples_per_line: usize,
    /// Sample size.
    pub format: SampleFormat,
    /// Lines per field.
    pub lines_per_field: usize,
    /// First line of each field that is used, zero-based.
    pub first_used: usize,
    /// Last line of each field that is used, zero-based, inclusive.
    pub last_used: usize,
    /// Television line number of the first line of each field, where the
    /// preset's source states it.
    pub first_line_numbers: Option<[u16; 2]>,
    /// Bytes at the end of each frame that may hold other data, such as the
    /// bt8x8 frame sequence number; they are not passed to the slicer.
    pub frame_trailer_bytes: usize,
}

/// Why a geometry cannot be used.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GeometryError {
    /// A line is empty or longer than [`MAX_LINE_BYTES`].
    LineLength,
    /// A field has no lines.
    NoLines,
    /// The used lines are not within the field.
    UsedLines,
    /// The frame trailer is longer than a line.
    Trailer,
}

impl RawGeometry {
    /// The geometry of a preset (REQ-INP-011 table).
    #[must_use]
    pub const fn preset(preset: Preset) -> Self {
        match preset {
            Preset::Bt8x8 => Self {
                sample_rate_hz: RATE_8_FSC_PAL,
                samples_per_line: SAMPLES_8_FSC,
                format: SampleFormat::U8,
                lines_per_field: LINES_16,
                first_used: 0,
                last_used: LINES_16 - 1,
                first_line_numbers: Some(BT8X8_FIRST_LINES),
                frame_trailer_bytes: BT8X8_SEQUENCE_BYTES,
            },
            Preset::Cx88 => Self {
                sample_rate_hz: RATE_8_FSC_PAL,
                samples_per_line: SAMPLES_8_FSC,
                format: SampleFormat::U8,
                lines_per_field: LINES_CX88,
                first_used: CX88_USED.0,
                last_used: CX88_USED.1,
                first_line_numbers: None,
                frame_trailer_bytes: 0,
            },
            Preset::Saa7131 => Self {
                sample_rate_hz: RATE_SAA7131,
                samples_per_line: SAMPLES_SAA7131,
                format: SampleFormat::U8,
                lines_per_field: LINES_16,
                first_used: 0,
                last_used: LINES_16 - 1,
                first_line_numbers: None,
                frame_trailer_bytes: 0,
            },
            Preset::Tbc => Self {
                sample_rate_hz: RATE_TBC,
                samples_per_line: SAMPLES_TBC,
                format: SampleFormat::U16Le,
                lines_per_field: LINES_TBC,
                first_used: TBC_USED.0,
                last_used: TBC_USED.1,
                first_line_numbers: None,
                frame_trailer_bytes: 0,
            },
            Preset::TbcVbi => Self {
                sample_rate_hz: RATE_TBC,
                samples_per_line: SAMPLES_TBC,
                format: SampleFormat::U16Le,
                lines_per_field: LINES_16,
                first_used: 0,
                last_used: LINES_16 - 1,
                first_line_numbers: None,
                frame_trailer_bytes: 0,
            },
        }
    }

    /// Bytes per line.
    #[must_use]
    pub const fn line_bytes(&self) -> usize {
        self.samples_per_line.saturating_mul(self.format.bytes_per_sample())
    }

    /// Lines per frame.
    #[must_use]
    pub const fn lines_per_frame(&self) -> usize {
        self.lines_per_field.saturating_mul(FIELDS_PER_FRAME)
    }

    /// Checks that the geometry is usable.
    pub const fn validate(&self) -> Result<(), GeometryError> {
        let bytes = self.line_bytes();
        if bytes == 0 || bytes > MAX_LINE_BYTES {
            return Err(GeometryError::LineLength);
        }
        if self.lines_per_field == 0 {
            return Err(GeometryError::NoLines);
        }
        if self.first_used > self.last_used || self.last_used >= self.lines_per_field {
            return Err(GeometryError::UsedLines);
        }
        if self.frame_trailer_bytes >= bytes {
            return Err(GeometryError::Trailer);
        }
        Ok(())
    }
}

impl Default for RawGeometry {
    fn default() -> Self {
        Self::preset(Preset::Bt8x8)
    }
}

/// A streaming reader of raw sampled video files.
#[derive(Clone, PartialEq, Debug)]
pub struct RawReader {
    geometry: RawGeometry,
    slicer: Slicer,
    line: [u8; MAX_LINE_BYTES],
    fill: usize,
    /// Line index within the frame of the line being filled.
    line_in_frame: usize,
    /// Frames completed.
    frames: u64,
}

impl RawReader {
    /// A reader for `geometry` that slices with `config`.
    pub fn new(geometry: RawGeometry, config: Config) -> Result<Self, GeometryError> {
        geometry.validate()?;
        Ok(Self {
            geometry,
            slicer: Slicer::new(config),
            line: [0; MAX_LINE_BYTES],
            fill: 0,
            line_in_frame: 0,
            frames: 0,
        })
    }

    /// The geometry.
    #[must_use]
    pub const fn geometry(&self) -> &RawGeometry {
        &self.geometry
    }

    /// The slicer, for its counters.
    #[must_use]
    pub const fn slicer(&self) -> &Slicer {
        &self.slicer
    }

    /// The slicer, to change its settings.
    pub const fn slicer_mut(&mut self) -> &mut Slicer {
        &mut self.slicer
    }

    /// Frames read completely.
    #[must_use]
    pub const fn frames(&self) -> u64 {
        self.frames
    }

    /// Drops a partly received line and starts again at a frame boundary.
    pub const fn reset(&mut self) {
        self.fill = 0;
        self.line_in_frame = 0;
    }

    /// Feeds the next chunk of the file, of any size, calling `on_packet`
    /// for each packet recovered.
    pub fn feed<P: FnMut(Packet, PacketMeta)>(&mut self, chunk: &[u8], on_packet: P) {
        self.feed_traced(chunk, on_packet, |_| {});
    }

    /// As [`RawReader::feed`], also passing slicer transitions to
    /// `on_trace` when tracing is on (REQ-FSM-010).
    pub fn feed_traced<P: FnMut(Packet, PacketMeta), T: FnMut(TraceEntry)>(
        &mut self,
        chunk: &[u8],
        mut on_packet: P,
        mut on_trace: T,
    ) {
        let line_bytes = self.geometry.line_bytes();
        let mut rest = chunk;
        while !rest.is_empty() {
            let wanted = line_bytes.saturating_sub(self.fill);
            let take = wanted.min(rest.len());
            let (head, tail) = rest.split_at(take);
            if let Some(target) = self.line.get_mut(self.fill..self.fill.saturating_add(take)) {
                target.copy_from_slice(head);
            }
            self.fill = self.fill.saturating_add(take);
            rest = tail;
            if self.fill >= line_bytes {
                self.line_complete(&mut on_packet, &mut on_trace);
                self.fill = 0;
            }
        }
    }

    fn line_complete<P: FnMut(Packet, PacketMeta), T: FnMut(TraceEntry)>(
        &mut self,
        on_packet: &mut P,
        on_trace: &mut T,
    ) {
        let geometry = self.geometry;
        let lines_per_field = geometry.lines_per_field.max(1);
        let field = self.line_in_frame.checked_div(lines_per_field).unwrap_or(0);
        let line_in_field = self.line_in_frame.checked_rem(lines_per_field).unwrap_or(0);
        let last_in_frame = self.line_in_frame.saturating_add(1) >= geometry.lines_per_frame();
        let used = line_in_field >= geometry.first_used && line_in_field <= geometry.last_used;
        if used {
            // The frame trailer overwrites the end of the frame's last line.
            let length = if last_in_frame {
                geometry.line_bytes().saturating_sub(geometry.frame_trailer_bytes)
            } else {
                geometry.line_bytes()
            };
            let fields = self.frames.saturating_mul(FIELDS_PER_FRAME as u64).saturating_add(field as u64);
            let line_number = geometry.first_line_numbers.and_then(|numbers| {
                let first = numbers.get(field).copied()?;
                first.checked_add(u16::try_from(line_in_field).ok()?)
            });
            let samples = self.line.get(..length).unwrap_or(&[]);
            let line = Line {
                samples,
                format: geometry.format,
                sample_rate_hz: geometry.sample_rate_hz,
                line: line_number,
                second_field: Some(field == 1),
                sync_offset_ns: None,
                timestamp_us: Some(fields.saturating_mul(FIELD_PERIOD_US)),
            };
            if let Ok(sliced) = self.slicer.slice_traced(&line, &mut *on_trace) {
                on_packet(sliced.packet, sliced.meta);
            }
        }
        if last_in_frame {
            self.line_in_frame = 0;
            self.frames = self.frames.saturating_add(1);
        } else {
            self.line_in_frame = self.line_in_frame.saturating_add(1);
        }
    }
}
