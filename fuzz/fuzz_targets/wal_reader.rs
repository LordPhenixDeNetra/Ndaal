//! Log reader of ndaal-wal (see ndaal_fuzz::wal_reader).

#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| ndaal_fuzz::wal_reader(data));
