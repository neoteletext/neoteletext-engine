//! The inverse MDCT (RFC 6716 §4.3.7) and the FFT under it, for the three sizes the profile
//! reaches: the 1920-point MDCT of a 20 ms frame's long block (a 480-point FFT), the 480-point
//! one of a 5 ms frame's (a 120-point FFT), and the 240-point one of every 2.5 ms block, short or
//! a frame of its own (a 60-point FFT). A 10 ms frame's 960-point MDCT is never reached.

pub mod fft;
pub mod mdct;

pub use mdct::{MDCT_240, MDCT_480, MDCT_1920, inverse_mdct, inverse_mdct_of_zeros};
