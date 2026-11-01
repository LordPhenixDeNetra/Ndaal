//! Masked CRC32C (FORMATS_DE_DONNEES.md section 1).

/// Added after the rotation: the LevelDB mask constant.
const MASK_DELTA: u32 = 0xa282_ead8;

/// CRC32C of `data`, masked as in LevelDB: a CRC computed over bytes that
/// contain their own CRC does not collide with it.
pub fn masked_crc(data: &[u8]) -> u32 {
    let crc = crc32c::crc32c(data);
    crc.rotate_right(15).wrapping_add(MASK_DELTA)
}
