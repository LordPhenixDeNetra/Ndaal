//! Codecs of ndaal-common (see ndaal_fuzz::common_codec).

#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| ndaal_fuzz::common_codec(data));
