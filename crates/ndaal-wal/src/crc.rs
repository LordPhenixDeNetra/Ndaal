//! Masked CRC32C (FORMATS_DE_DONNEES.md section 1).

/// Added after the rotation: the LevelDB mask constant.
const MASK_DELTA: u32 = 0xa282_ead8;

/// CRC32C of `data`, masked as in LevelDB: a CRC computed over bytes that
/// contain their own CRC does not collide with it.
pub fn masked_crc(data: &[u8]) -> u32 {
    mask(crc32c::crc32c(data))
}

/// [`masked_crc`] of `data` preceded by a seed of the file's full recycle
/// epoch and number (ADR-034): a frame of another life of the file, or of
/// another file, fails it whatever the low bits of its epoch.
pub fn seeded_crc(epoch: u64, file_no: u64, data: &[u8]) -> u32 {
    let mut seed = [0u8; 16];
    seed[..8].copy_from_slice(&epoch.to_le_bytes());
    seed[8..].copy_from_slice(&file_no.to_le_bytes());
    mask(crc32c::crc32c_append(crc32c::crc32c(&seed), data))
}

fn mask(crc: u32) -> u32 {
    crc.rotate_right(15).wrapping_add(MASK_DELTA)
}
