//! Texture checksumming utilities used to produce a secondary key for mesh
//! identification (see the mod_render select path).
//!
//! The checksum is a plain CRC32 over a fixed-size center window of the raw
//! texture bytes at mip 0. For uncompressed formats the window is
//! `sample_size x sample_size` pixels, centered in the image (or the whole
//! image if it is smaller than the sample size). For block-compressed (BC)
//! formats the window is rounded to 4-pixel block boundaries and hashed as
//! a set of block rows.
//!
//! The algorithm identifier written into the snapshot meta file is
//! `crc32-center<N>`, e.g. `crc32-center64`. Changing the algorithm later
//! should change this identifier so existing mod yamls aren't silently
//! invalidated.

use crc32fast::Hasher;

/// Default sample window, in pixels on a side.
pub const DEFAULT_SAMPLE_SIZE: u32 = 64;

/// Algorithm identifier written to the snapshot meta file.
pub fn algorithm_name(sample_size: u32) -> String {
    format!("crc32-center{}", sample_size)
}

/// Compute a CRC32 over the centered `sample_size x sample_size` region of an
/// uncompressed texture surface.
///
/// * `data` - raw bytes of mip 0
/// * `width`, `height` - texture dimensions in pixels
/// * `row_pitch` - bytes per row (may exceed `width * bpp_bytes` due to padding)
/// * `bpp_bytes` - bytes per pixel (e.g. 4 for RGBA8, 2 for R5G6B5)
/// * `sample_size` - desired window size in pixels
///
/// Returns `None` if the inputs are degenerate (zero dims, too-small pitch,
/// truncated buffer).
pub fn compute_uncompressed(
    data: &[u8],
    width: u32,
    height: u32,
    row_pitch: u32,
    bpp_bytes: u32,
    sample_size: u32,
) -> Option<u32> {
    if width == 0 || height == 0 || bpp_bytes == 0 || sample_size == 0 {
        return None;
    }
    let min_pitch = width.checked_mul(bpp_bytes)?;
    if row_pitch < min_pitch {
        return None;
    }

    // Clamp the window to the image size, then center it.
    let win_w = sample_size.min(width);
    let win_h = sample_size.min(height);
    let x0 = (width - win_w) / 2;
    let y0 = (height - win_h) / 2;

    let row_bytes = (win_w as usize) * (bpp_bytes as usize);
    let start_x_bytes = (x0 as usize) * (bpp_bytes as usize);

    // Bounds-check the last byte we'd read.
    let last_row = (y0 as usize) + (win_h as usize) - 1;
    let last_byte = last_row
        .checked_mul(row_pitch as usize)?
        .checked_add(start_x_bytes)?
        .checked_add(row_bytes)?;
    if last_byte > data.len() {
        return None;
    }

    let mut h = Hasher::new();
    for r in 0..(win_h as usize) {
        let row_start = ((y0 as usize) + r) * (row_pitch as usize) + start_x_bytes;
        h.update(&data[row_start..row_start + row_bytes]);
    }
    Some(h.finalize())
}

/// Compute a CRC32 over the centered region of a block-compressed texture
/// surface (BC1-BC7, i.e. DXT / BPTC). The window is rounded to 4-pixel block
/// boundaries.
///
/// * `data` - raw bytes of mip 0
/// * `width`, `height` - texture dimensions in pixels (not blocks)
/// * `row_pitch` - bytes per row of blocks
/// * `block_bytes` - bytes per 4x4 block (8 for BC1/BC4, 16 for BC2/BC3/BC5/BC6/BC7)
/// * `sample_size` - desired window size in pixels
pub fn compute_block_compressed(
    data: &[u8],
    width: u32,
    height: u32,
    row_pitch: u32,
    block_bytes: u32,
    sample_size: u32,
) -> Option<u32> {
    if width == 0 || height == 0 || block_bytes == 0 || sample_size == 0 {
        return None;
    }
    // Block-compressed dims are in 4x4 blocks (rounded up).
    let blocks_w = (width + 3) / 4;
    let blocks_h = (height + 3) / 4;
    let min_pitch = blocks_w.checked_mul(block_bytes)?;
    if row_pitch < min_pitch {
        return None;
    }

    // Window, clamped to image, rounded to blocks.
    let win_w_px = sample_size.min(width);
    let win_h_px = sample_size.min(height);
    let win_w_blocks = ((win_w_px + 3) / 4).max(1);
    let win_h_blocks = ((win_h_px + 3) / 4).max(1);
    let win_w_blocks = win_w_blocks.min(blocks_w);
    let win_h_blocks = win_h_blocks.min(blocks_h);

    let x0_blocks = (blocks_w - win_w_blocks) / 2;
    let y0_blocks = (blocks_h - win_h_blocks) / 2;

    let row_bytes = (win_w_blocks as usize) * (block_bytes as usize);
    let start_x_bytes = (x0_blocks as usize) * (block_bytes as usize);

    let last_row = (y0_blocks as usize) + (win_h_blocks as usize) - 1;
    let last_byte = last_row
        .checked_mul(row_pitch as usize)?
        .checked_add(start_x_bytes)?
        .checked_add(row_bytes)?;
    if last_byte > data.len() {
        return None;
    }

    let mut h = Hasher::new();
    for r in 0..(win_h_blocks as usize) {
        let row_start = ((y0_blocks as usize) + r) * (row_pitch as usize) + start_x_bytes;
        h.update(&data[row_start..row_start + row_bytes]);
    }
    Some(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_rgba8_64x64() {
        // Build a 64x64 RGBA8 buffer with a deterministic pattern, tight pitch.
        let w = 64u32;
        let h = 64u32;
        let bpp = 4u32;
        let pitch = w * bpp;
        let mut data = vec![0u8; (pitch * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = (y * pitch + x * bpp) as usize;
                data[i] = (x as u8).wrapping_mul(3);
                data[i + 1] = (y as u8).wrapping_mul(5);
                data[i + 2] = ((x ^ y) as u8).wrapping_mul(7);
                data[i + 3] = 0xff;
            }
        }
        let a = compute_uncompressed(&data, w, h, pitch, bpp, 64).expect("hash a");
        let b = compute_uncompressed(&data, w, h, pitch, bpp, 64).expect("hash b");
        assert_eq!(a, b, "deterministic");
        // Different data should produce a different hash with overwhelming probability.
        data[0] ^= 0x01;
        let c = compute_uncompressed(&data, w, h, pitch, bpp, 64).expect("hash c");
        assert_ne!(a, c);
    }

    #[test]
    fn smaller_than_window_clamps() {
        // 16x16 RGBA8, request 64 window -> should use the whole image.
        let w = 16u32;
        let h = 16u32;
        let bpp = 4u32;
        let pitch = w * bpp;
        let mut data = vec![0u8; (pitch * h) as usize];
        for i in 0..data.len() {
            data[i] = i as u8;
        }
        let a = compute_uncompressed(&data, w, h, pitch, bpp, 64).expect("hash");
        // Hashing with an explicit 16 window over the same image should match.
        let b = compute_uncompressed(&data, w, h, pitch, bpp, 16).expect("hash16");
        assert_eq!(a, b);
    }

    #[test]
    fn pitch_padding_ignored() {
        // A 4x4 RGBA8 image with 32-byte padded row pitch: hash must ignore padding.
        let w = 4u32;
        let h = 4u32;
        let bpp = 4u32;
        let tight = w * bpp; // 16
        let padded = 32u32;
        let mut tight_buf = vec![0u8; (tight * h) as usize];
        let mut padded_buf = vec![0xaa_u8; (padded * h) as usize]; // fill padding with junk
        for y in 0..h {
            for x in 0..(tight as usize) {
                let b = ((y as usize * 13) + x) as u8;
                tight_buf[y as usize * tight as usize + x] = b;
                padded_buf[y as usize * padded as usize + x] = b;
            }
        }
        let a = compute_uncompressed(&tight_buf, w, h, tight, bpp, 4).expect("a");
        let b = compute_uncompressed(&padded_buf, w, h, padded, bpp, 4).expect("b");
        assert_eq!(a, b, "padding bytes should not affect the hash");
    }

    #[test]
    fn rejects_truncated_buffer() {
        let w = 64u32;
        let h = 64u32;
        let bpp = 4u32;
        let pitch = w * bpp;
        let data = vec![0u8; (pitch * (h - 1)) as usize]; // missing last row
        assert!(compute_uncompressed(&data, w, h, pitch, bpp, 64).is_none());
    }

    #[test]
    fn rejects_bad_inputs() {
        let data = vec![0u8; 1024];
        assert!(compute_uncompressed(&data, 0, 16, 64, 4, 64).is_none());
        assert!(compute_uncompressed(&data, 16, 0, 64, 4, 64).is_none());
        assert!(compute_uncompressed(&data, 16, 16, 0, 4, 64).is_none()); // pitch < min
        assert!(compute_uncompressed(&data, 16, 16, 64, 0, 64).is_none());
        assert!(compute_uncompressed(&data, 16, 16, 64, 4, 0).is_none());
    }

    #[test]
    fn bc1_block_compressed() {
        // 64x64 BC1: 16x16 blocks, 8 bytes each = 128 bytes per block row.
        let w = 64u32;
        let h = 64u32;
        let block_bytes = 8u32;
        let blocks_w = (w + 3) / 4;
        let blocks_h = (h + 3) / 4;
        let pitch = blocks_w * block_bytes;
        let mut data = vec![0u8; (pitch * blocks_h) as usize];
        for i in 0..data.len() {
            data[i] = (i as u8).wrapping_mul(17);
        }
        let a = compute_block_compressed(&data, w, h, pitch, block_bytes, 64).expect("hash");
        let b = compute_block_compressed(&data, w, h, pitch, block_bytes, 64).expect("hash");
        assert_eq!(a, b);
        // Mutating outside the center should not matter when the window is smaller
        // than the image; here sample=64 equals image, so any change matters.
        data[0] ^= 1;
        let c = compute_block_compressed(&data, w, h, pitch, block_bytes, 64).expect("hash");
        assert_ne!(a, c);
    }

    #[test]
    fn algo_name_format() {
        assert_eq!(algorithm_name(64), "crc32-center64");
        assert_eq!(algorithm_name(32), "crc32-center32");
    }
}
