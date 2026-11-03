//! Segment envelope of ndaal-storage (see ndaal_fuzz::segment_reader).

#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| ndaal_fuzz::segment_reader(data));
