//! PNG writer tests, checked against an independent fixed-Huffman inflater.
#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::comparison_chain)]

use super::*;
use crate::palette::{CLUT0, palette};
use std::vec;
use std::vec::Vec;

/// A minimal inflate for fixed-Huffman blocks, to check the encoder
/// independently of it.
fn inflate_fixed(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut bit = 0usize;
    let read = |count: u32, bit: &mut usize| -> u32 {
        let mut value = 0;
        for k in 0..count {
            let b = (data[*bit / 8] >> (*bit % 8)) & 1;
            value |= u32::from(b) << k;
            *bit += 1;
        }
        value
    };
    let read_code = |bit: &mut usize, count: u32| -> u32 {
        let mut value = 0;
        for _ in 0..count {
            value = (value << 1) | u32::from((data[*bit / 8] >> (*bit % 8)) & 1);
            *bit += 1;
        }
        value
    };
    assert_eq!(read(3, &mut bit), 0b011);
    loop {
        let mut code = read_code(&mut bit, 7);
        let symbol = if code <= 0x17 {
            code + 256
        } else {
            code = (code << 1) | read_code(&mut bit, 1);
            if (0x30..=0xBF).contains(&code) {
                code - 0x30
            } else if (0xC0..=0xC7).contains(&code) {
                code - 0xC0 + 280
            } else {
                code = (code << 1) | read_code(&mut bit, 1);
                code - 0x190 + 144
            }
        };
        if symbol < 256 {
            out.push(symbol as u8);
        } else if symbol == 256 {
            break;
        } else {
            let (base, extra) = LENGTH_BASE[(symbol - 257) as usize];
            let length = usize::from(base) + read(extra, &mut bit) as usize;
            let dcode = read_code(&mut bit, 5) as usize;
            let (dbase, dextra) = DISTANCE_BASE[dcode];
            let distance = (dbase + read(dextra, &mut bit)) as usize;
            for _ in 0..length {
                out.push(out[out.len() - distance]);
            }
        }
    }
    out
}

#[test]
fn round_trip_through_an_independent_inflate() {
    let (width, height) = (37usize, 9usize);
    let image: Vec<u8> = (0..width * height).map(|i| if (i / width) % 3 == 0 { 7 } else { (i % 5) as u8 }).collect();
    let mut sink = VecSink(Vec::new());
    let mut scratch = vec![0u8; 2 * (width + 1)];
    let colours = palette(&CLUT0, Rgba::TRANSPARENT, true, false);
    write(
        &mut sink,
        width,
        height,
        &colours,
        &mut |y, row: &mut [u8]| row.copy_from_slice(&image[y * width..(y + 1) * width]),
        &mut scratch,
    )
    .unwrap();
    let out = sink.0;
    assert_eq!(&out[..8], &SIGNATURE);
    // Gather IDAT data.
    let mut position = 8;
    let mut zlib = Vec::new();
    while position < out.len() {
        let length = u32::from_be_bytes(out[position..position + 4].try_into().unwrap()) as usize;
        let kind = &out[position + 4..position + 8];
        let body = &out[position + 8..position + 8 + length];
        let crc = u32::from_be_bytes(out[position + 8 + length..position + 12 + length].try_into().unwrap());
        assert_eq!(crc, !crc_update(crc_update(u32::MAX, kind), body));
        if kind == b"IDAT" {
            zlib.extend_from_slice(body);
        }
        position += 12 + length;
    }
    assert_eq!(&zlib[..2], &ZLIB_HEADER);
    let raw = inflate_fixed(&zlib[2..zlib.len() - 4]);
    let mut expected = Vec::new();
    for y in 0..height {
        expected.push(0);
        expected.extend_from_slice(&image[y * width..(y + 1) * width]);
    }
    assert_eq!(raw, expected);
}

struct VecSink(Vec<u8>);

impl Sink for VecSink {
    type Error = ();
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}
