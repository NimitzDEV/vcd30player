//! Decoder for VCD 3.0 .YBM (YUVBT.601 Bitmap) image format.
//!
//! Binary layout:
//! - 0x00..0x10 (16 bytes): Magic string `<YUVBMP>1.15a\0\xF7\xBF`
//! - 0x10..0x1E (14 bytes): BITMAPFILEHEADER (Big-Endian 'BM', file size, etc.)
//! - 0x1E..0x20 ( 2 bytes): Padding (0x00, 0x00)
//! - 0x20..0x48 (40 bytes): BITMAPINFOHEADER (Big-Endian biSize=40, width, height, 8bpp, etc.)
//! - 0x48..0x448 (1024 bytes): 256 * 4 YUV palette (reserved, Cr^0x80, Cb^0x80, Y^0x80)
//! - 0x448..End: 8-bit indexed pixels in top-down raster scan order (width * height bytes)

use std::fmt;

pub const YBM_MAGIC: &[u8; 8] = b"<YUVBMP>";
pub const PALETTE_OFFSET: usize = 0x48;
pub const PIXELS_OFFSET: usize = 0x448;
pub const PALETTE_COLORS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YbmError {
    DataTooSmall(usize),
    InvalidMagic,
    InvalidBmpHeader,
    InvalidDimensions(u32, u32),
    IncompletePixelData { expected: usize, actual: usize },
}

impl fmt::Display for YbmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            YbmError::DataTooSmall(len) => {
                write!(f, "Data too small for YBM header: {} bytes", len)
            }
            YbmError::InvalidMagic => write!(f, "Invalid YBM magic header (expected <YUVBMP>)"),
            YbmError::InvalidBmpHeader => write!(f, "Invalid embedded BMP header"),
            YbmError::InvalidDimensions(w, h) => write!(f, "Invalid image dimensions: {}x{}", w, h),
            YbmError::IncompletePixelData { expected, actual } => {
                write!(
                    f,
                    "Incomplete pixel data: expected {} bytes, found {}",
                    expected, actual
                )
            }
        }
    }
}

impl std::error::Error for YbmError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YbmImage {
    pub width: u32,
    pub height: u32,
    pub palette: [RgbColor; PALETTE_COLORS],
    pub pixels: Vec<u8>,
}

#[inline]
fn clamp_u8(v: f64) -> u8 {
    if v <= 0.0 {
        0
    } else if v >= 255.0 {
        255
    } else {
        v.round() as u8
    }
}

/// Decodes an ITU-R BT.601 YUV quadruplet from raw palette memory:
/// Raw bytes: [b0, b1, b2, b3] = [reserved, Cr ^ 0x80, Cb ^ 0x80, Y ^ 0x80]
/// Algorithm matching AUTORUN.EXE 0x407cc0.
pub fn decode_yuv_entry(raw: &[u8]) -> RgbColor {
    let y = (raw[3] ^ 0x80) as f64;
    let cr = ((raw[1] ^ 0x80) as i32 - 128) as f64;
    let cb = ((raw[2] ^ 0x80) as i32 - 128) as f64;

    let r = clamp_u8(y + 1.402 * cr);
    let g = clamp_u8(y - 0.71414 * cr - 0.34414 * cb);
    let b = clamp_u8(y + 1.772 * cb);

    RgbColor { r, g, b }
}

impl YbmImage {
    /// Decodes a .YBM image from binary data.
    pub fn decode(bytes: &[u8]) -> Result<Self, YbmError> {
        if bytes.len() < PIXELS_OFFSET {
            return Err(YbmError::DataTooSmall(bytes.len()));
        }

        if !bytes.starts_with(YBM_MAGIC) {
            return Err(YbmError::InvalidMagic);
        }

        // BITMAPINFOHEADER starts at offset 0x20
        let bi_size = u32::from_be_bytes(bytes[0x20..0x24].try_into().unwrap());
        if bi_size != 40 {
            return Err(YbmError::InvalidBmpHeader);
        }

        let width = u32::from_be_bytes(bytes[0x24..0x28].try_into().unwrap());
        let height = u32::from_be_bytes(bytes[0x28..0x2c].try_into().unwrap());

        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err(YbmError::InvalidDimensions(width, height));
        }

        // Decode 256 YUV palette entries at 0x48..0x448
        let mut palette = [RgbColor { r: 0, g: 0, b: 0 }; PALETTE_COLORS];
        for (i, entry) in palette.iter_mut().enumerate() {
            let offset = PALETTE_OFFSET + i * 4;
            *entry = decode_yuv_entry(&bytes[offset..offset + 4]);
        }

        let expected_pixels = (width * height) as usize;
        let actual_pixels = bytes.len() - PIXELS_OFFSET;
        if actual_pixels < expected_pixels {
            return Err(YbmError::IncompletePixelData {
                expected: expected_pixels,
                actual: actual_pixels,
            });
        }

        let pixels = bytes[PIXELS_OFFSET..PIXELS_OFFSET + expected_pixels].to_vec();

        Ok(Self {
            width,
            height,
            palette,
            pixels,
        })
    }

    /// Converts the indexed image into 32-bit RGBA pixels (row-major top-down).
    pub fn to_rgba8(&self) -> Vec<u8> {
        let mut rgba = Vec::with_capacity((self.width * self.height * 4) as usize);
        for &idx in &self.pixels {
            let color = self.palette[idx as usize];
            rgba.push(color.r);
            rgba.push(color.g);
            rgba.push(color.b);
            rgba.push(255);
        }
        rgba
    }

    /// Converts the indexed image into 24-bit RGB pixels (row-major top-down).
    pub fn to_rgb8(&self) -> Vec<u8> {
        let mut rgb = Vec::with_capacity((self.width * self.height * 3) as usize);
        for &idx in &self.pixels {
            let color = self.palette[idx as usize];
            rgb.push(color.r);
            rgb.push(color.g);
            rgb.push(color.b);
        }
        rgb
    }

    /// Blits a sub-image or sprite onto a target 32-bit RGBA canvas with optional transparent color.
    pub fn blit_to_rgba_canvas(
        &self,
        canvas: &mut [u8],
        canvas_width: u32,
        canvas_height: u32,
        dst_x: i32,
        dst_y: i32,
        transparent_idx: Option<u8>,
    ) {
        for src_y in 0..self.height {
            let target_y = dst_y + src_y as i32;
            if target_y < 0 || target_y >= canvas_height as i32 {
                continue;
            }

            for src_x in 0..self.width {
                let target_x = dst_x + src_x as i32;
                if target_x < 0 || target_x >= canvas_width as i32 {
                    continue;
                }

                let src_idx = (src_y * self.width + src_x) as usize;
                let color_idx = self.pixels[src_idx];

                if let Some(t_idx) = transparent_idx {
                    if color_idx == t_idx {
                        continue;
                    }
                }

                let color = self.palette[color_idx as usize];
                let canvas_idx = ((target_y as u32 * canvas_width + target_x as u32) * 4) as usize;

                canvas[canvas_idx] = color.r;
                canvas[canvas_idx + 1] = color.g;
                canvas[canvas_idx + 2] = color.b;
                canvas[canvas_idx + 3] = 255;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yuv_decode_pure_color() {
        // Test Y = 16 (stored as 16 ^ 0x80 = 0x90)
        // Cr = 0 (stored as (0 + 128) ^ 0x80 = 128 ^ 0x80 = 0x00)
        // Cb = 0 (stored as (0 + 128) ^ 0x80 = 128 ^ 0x80 = 0x00)
        // With Cr=0, Cb=0 -> R = 16, G = 16, B = 16
        let raw = [0x00, 0x00, 0x00, 0x90];
        let rgb = decode_yuv_entry(&raw);
        assert_eq!(
            rgb,
            RgbColor {
                r: 16,
                g: 16,
                b: 16
            }
        );
    }

    #[test]
    fn test_ybm_header_validation() {
        let bad_data = b"NOT_YBM_FILE_DATA";
        assert_eq!(
            YbmImage::decode(bad_data),
            Err(YbmError::DataTooSmall(bad_data.len()))
        );

        let mut fake = vec![0u8; PIXELS_OFFSET + 10];
        assert_eq!(YbmImage::decode(&fake), Err(YbmError::InvalidMagic));

        fake[..8].copy_from_slice(YBM_MAGIC);
        assert_eq!(YbmImage::decode(&fake), Err(YbmError::InvalidBmpHeader));
    }
}
