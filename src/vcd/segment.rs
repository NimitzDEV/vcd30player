//! VCD 2.0 Segment Play Item (`/SEGMENT/ITEMxxxx.DAT`) discovery and decoder.
//!
//! Segment items in VCD 2.0 typically hold high-resolution still picture menus
//! (e.g. 704x576 PAL or 704x480 NTSC) or short audio/video clips.
//! Standard item IDs in PSD range from 1000..=9999, corresponding to ITEM0001.DAT..=ITEM9000.DAT.

use std::path::{Path, PathBuf};

use super::detector::find_path_ci;
use crate::video::decoder::MpegDecoder;

pub const CANVAS_WIDTH: usize = 352;
pub const CANVAS_HEIGHT: usize = 288;

/// Resolves the filesystem path for a segment item ID (e.g. 1000 -> ITEM0001.DAT).
pub fn resolve_segment_path(disc_root: &Path, item_id: u16) -> Option<PathBuf> {
    if item_id < 1000 {
        return None;
    }
    let seg_idx = item_id - 999;
    let file_name = format!("ITEM{:04}.DAT", seg_idx);
    find_path_ci(disc_root, &["SEGMENT", &file_name])
}

/// Decodes the primary still picture frame from a `/SEGMENT/ITEMxxxx.DAT` file.
/// Returns `(width, height, rgba_buffer)`.
pub fn decode_segment_frame(dat_path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let bytes = std::fs::read(dat_path)
        .map_err(|e| format!("Failed to read segment file {}: {}", dat_path.display(), e))?;

    let mut decoder = MpegDecoder::from_bytes(&bytes)?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 {
        return Err(format!("Invalid video dimensions ({}x{}) in {}", w, h, dat_path.display()));
    }

    let mut rgba = vec![0u8; (w * h * 4) as usize];
    if decoder.decode_video_frame(&mut rgba).is_none() {
        return Err(format!("Failed to decode video frame from {}", dat_path.display()));
    }

    Ok((w, h, rgba))
}

/// Downsamples a 704x576 PAL still picture menu directly to the standard 352x288 canvas
/// using 2x2 area pixel averaging for pristine image clarity without aliasing.
pub fn downsample_704x576_to_352x288(src: &[u8], dst: &mut [u8]) {
    for y in 0..CANVAS_HEIGHT {
        for x in 0..CANVAS_WIDTH {
            let src_y = y * 2;
            let src_x = x * 2;
            let idx00 = (src_y * 704 + src_x) * 4;
            let idx01 = (src_y * 704 + (src_x + 1)) * 4;
            let idx10 = ((src_y + 1) * 704 + src_x) * 4;
            let idx11 = ((src_y + 1) * 704 + (src_x + 1)) * 4;

            let dst_idx = (y * CANVAS_WIDTH + x) * 4;
            for c in 0..3 {
                let sum = src[idx00 + c] as u32
                    + src[idx01 + c] as u32
                    + src[idx10 + c] as u32
                    + src[idx11 + c] as u32;
                dst[dst_idx + c] = (sum / 4) as u8;
            }
            dst[dst_idx + 3] = 255;
        }
    }
}

/// Blits and scales any decoded RGBA image onto the 352x288 target canvas.
pub fn blit_segment_to_canvas(src_w: u32, src_h: u32, src_rgba: &[u8], dst_canvas: &mut [u8]) {
    if src_w as usize == CANVAS_WIDTH && src_h as usize == CANVAS_HEIGHT {
        dst_canvas.copy_from_slice(src_rgba);
    } else if src_w == 704 && src_h == 576 {
        downsample_704x576_to_352x288(src_rgba, dst_canvas);
    } else {
        // Nearest-neighbor fallback scaling
        for dst_y in 0..CANVAS_HEIGHT {
            let src_y = ((dst_y as u32 * src_h) / CANVAS_HEIGHT as u32).min(src_h.saturating_sub(1));
            for dst_x in 0..CANVAS_WIDTH {
                let src_x = ((dst_x as u32 * src_w) / CANVAS_WIDTH as u32).min(src_w.saturating_sub(1));
                let src_idx = ((src_y * src_w + src_x) * 4) as usize;
                let dst_idx = (dst_y * CANVAS_WIDTH + dst_x) * 4;
                if src_idx + 4 <= src_rgba.len() && dst_idx + 4 <= dst_canvas.len() {
                    dst_canvas[dst_idx..dst_idx + 4].copy_from_slice(&src_rgba[src_idx..src_idx + 4]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downsample_pure_colors() {
        let mut src = vec![0u8; 704 * 576 * 4];
        // Fill with white
        for pixel in src.chunks_exact_mut(4) {
            pixel[0] = 200;
            pixel[1] = 100;
            pixel[2] = 50;
            pixel[3] = 255;
        }

        let mut dst = vec![0u8; 352 * 288 * 4];
        downsample_704x576_to_352x288(&src, &mut dst);

        assert_eq!(dst[0], 200);
        assert_eq!(dst[1], 100);
        assert_eq!(dst[2], 50);
        assert_eq!(dst[3], 255);
    }

    #[test]
    fn test_resolve_segment_path() {
        let root = Path::new("dummy_root");
        assert_eq!(resolve_segment_path(root, 999), None);
    }
}
