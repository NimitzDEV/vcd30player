//! Parser for VCD 3.0 Compiled HTML `<COMPHTML>` (.CHM) interactive page format.
//!
//! Binary layout:
//! - 0x00..0x0C (12 bytes): Magic string `<COMPHTML>\0\0`
//! - 0x0C..0xDC (208 bytes): Header (dimensions, palette count, title, author, license signature)
//! - 0xDC..0x4DC (1024 bytes): 256 * 4 YUV palette (BT.601 converted)
//! - 0x4DC..End: Chunk stream with 12-byte headers [type, size, flags]
//! - End tag: `</COMPHTML>`

use crate::assets::ybm::{PALETTE_COLORS, RgbColor, decode_yuv_entry};
use std::fmt;

pub const COMPHTML_MAGIC: &[u8; 10] = b"<COMPHTML>";
pub const COMPHTML_END_TAG: &[u8; 11] = b"</COMPHTML>";

pub const CHUNK_TYPE_TITLE: u32 = 0;
pub const CHUNK_TYPE_BODY_STYLE: u32 = 1;
pub const CHUNK_TYPE_IMG: u32 = 3;
pub const CHUNK_TYPE_MAP: u32 = 11;
pub const CHUNK_TYPE_HREF: u32 = 13;
pub const CHUNK_TYPE_BGSOUND: u32 = 15;
pub const CHUNK_TYPE_VCDSCRIPT: u32 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChmError {
    DataTooSmall(usize),
    InvalidMagic,
    InvalidHeader,
    InvalidDimensions(u32, u32),
    CorruptedChunk(String),
}

impl fmt::Display for ChmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChmError::DataTooSmall(len) => write!(f, "Data too small for CHM: {} bytes", len),
            ChmError::InvalidMagic => write!(f, "Invalid CHM magic header (expected <COMPHTML>)"),
            ChmError::InvalidHeader => write!(f, "Invalid CHM metadata header"),
            ChmError::InvalidDimensions(w, h) => write!(f, "Invalid CHM dimensions: {}x{}", w, h),
            ChmError::CorruptedChunk(msg) => write!(f, "Corrupted chunk: {}", msg),
        }
    }
}

impl std::error::Error for ChmError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapArea {
    pub area_id: u32,
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub target: String,
    pub script_entry_line: Option<u32>,
}

impl MapArea {
    /// Returns the exact bounding box `(min_x, min_y, max_x, max_y)` as authored.
    pub fn raw_bounds(&self) -> (i32, i32, i32, i32) {
        let min_x = self.x1.min(self.x2);
        let max_x = self.x1.max(self.x2);
        let min_y = self.y1.min(self.y2);
        let max_y = self.y1.max(self.y2);
        (min_x, min_y, max_x, max_y)
    }

    /// Whether this hotspot is a degenerate zero-area / point anchor (e.g. x1 == x2 && y1 == y2).
    pub fn is_point_hotspot(&self) -> bool {
        let (min_x, min_y, max_x, max_y) = self.raw_bounds();
        (max_x - min_x) <= 3 && (max_y - min_y) <= 3
    }

    /// Returns display bounds for debug overlay rendering.
    /// Point hotspots are rendered with an indicator box (e.g. +-12px),
    /// while real button hotspots are rendered with exact boundaries.
    pub fn display_bounds(&self) -> (i32, i32, i32, i32) {
        let (min_x, min_y, max_x, max_y) = self.raw_bounds();
        if self.is_point_hotspot() {
            let mid_x = (min_x + max_x) / 2;
            let mid_y = (min_y + max_y) / 2;
            (
                (mid_x - 12).max(0),
                (mid_y - 12).max(0),
                (mid_x + 12).min(352),
                (mid_y + 12).min(288),
            )
        } else {
            (min_x, min_y, max_x, max_y)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageElement {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub usemap: Option<String>,
    pub filename: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkPayload {
    Title(String),
    BodyStyle {
        font_name: String,
        source_path: String,
    },
    Image(ImageElement),
    MapHotspots {
        map_name: String,
        areas: Vec<MapArea>,
    },
    BgSound(String),
    VcdScript(String),
    Raw {
        chunk_type: u32,
        flags: u32,
        data: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
pub struct CompHtmlDoc {
    pub title: String,
    pub author: String,
    pub width: u32,
    pub height: u32,
    pub bg_color_idx: u32,
    pub palette_count: u32,
    pub palette: [RgbColor; PALETTE_COLORS],
    pub chunks: Vec<ChunkPayload>,
}

fn extract_null_terminated_str(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

impl CompHtmlDoc {
    /// Parses a `<COMPHTML>` .CHM file from raw bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, ChmError> {
        if bytes.len() < 220 {
            return Err(ChmError::DataTooSmall(bytes.len()));
        }

        if !bytes.starts_with(COMPHTML_MAGIC) {
            return Err(ChmError::InvalidMagic);
        }

        // Header: 12..220 (208 bytes)
        let header = &bytes[12..220];
        let title = extract_null_terminated_str(&header[0x20..0x60]);
        let author = extract_null_terminated_str(&header[0x60..0x80]);

        let width = u32::from_be_bytes(header[0x80..0x84].try_into().unwrap());
        let height = u32::from_be_bytes(header[0x84..0x88].try_into().unwrap());
        let bg_color_idx = u32::from_be_bytes(header[0x88..0x8c].try_into().unwrap());
        let palette_count = u32::from_be_bytes(header[0x8c..0x90].try_into().unwrap());

        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err(ChmError::InvalidDimensions(width, height));
        }

        let pal_bytes = (palette_count.min(256) * 4) as usize;
        let pal_start = 220;
        let pal_end = pal_start + pal_bytes;
        if bytes.len() < pal_end {
            return Err(ChmError::DataTooSmall(bytes.len()));
        }

        let mut palette = [RgbColor { r: 0, g: 0, b: 0 }; PALETTE_COLORS];
        for i in 0..palette_count.min(256) as usize {
            let off = pal_start + i * 4;
            palette[i] = decode_yuv_entry(&bytes[off..off + 4]);
        }

        // Parse chunks starting at pal_end
        let mut chunks = Vec::new();
        let mut offset = pal_end;

        while offset + 12 <= bytes.len() {
            // Check for </COMPHTML>
            if bytes[offset..].starts_with(COMPHTML_END_TAG) {
                break;
            }

            let c_type = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap());
            let c_size =
                u32::from_be_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
            let c_flags = u32::from_be_bytes(bytes[offset + 8..offset + 12].try_into().unwrap());

            offset += 12;
            if offset + c_size > bytes.len() {
                // Chunk size overflows remaining data
                break;
            }

            let c_data = &bytes[offset..offset + c_size];
            offset += c_size;

            let payload = match c_type {
                CHUNK_TYPE_TITLE => {
                    let s = extract_null_terminated_str(c_data);
                    ChunkPayload::Title(s)
                }
                CHUNK_TYPE_BODY_STYLE => {
                    let font_name = if c_data.len() > 0x10 {
                        extract_null_terminated_str(&c_data[..0x20])
                    } else {
                        "Courier".to_string()
                    };
                    let source_path = if c_data.len() > 0x40 {
                        extract_null_terminated_str(&c_data[0x40..])
                    } else {
                        String::new()
                    };
                    ChunkPayload::BodyStyle {
                        font_name,
                        source_path,
                    }
                }
                CHUNK_TYPE_IMG => {
                    if c_data.len() >= 0x20 {
                        let x = i32::from_be_bytes(c_data[0x00..0x04].try_into().unwrap());
                        let y = i32::from_be_bytes(c_data[0x04..0x08].try_into().unwrap());
                        let w = u32::from_be_bytes(c_data[0x08..0x0c].try_into().unwrap());
                        let h = u32::from_be_bytes(c_data[0x0c..0x10].try_into().unwrap());

                        let usemap_str = extract_null_terminated_str(&c_data[0x1c..]);
                        let usemap = if !usemap_str.is_empty() {
                            Some(usemap_str)
                        } else {
                            None
                        };

                        let mut filename = String::new();
                        // Find .YBM or .BMP extension in chunk data
                        for (i, w) in c_data.windows(4).enumerate() {
                            if w.eq_ignore_ascii_case(b".YBM") || w.eq_ignore_ascii_case(b".BMP") {
                                let mut start = i;
                                while start > 0
                                    && c_data[start - 1] >= 32
                                    && c_data[start - 1] < 127
                                {
                                    start -= 1;
                                }
                                filename =
                                    String::from_utf8_lossy(&c_data[start..i + 4]).to_string();
                                break;
                            }
                        }

                        ChunkPayload::Image(ImageElement {
                            x,
                            y,
                            width: w,
                            height: h,
                            usemap,
                            filename,
                        })
                    } else {
                        ChunkPayload::Raw {
                            chunk_type: c_type,
                            flags: c_flags,
                            data: c_data.to_vec(),
                        }
                    }
                }
                CHUNK_TYPE_MAP => {
                    let map_name = extract_null_terminated_str(&c_data[..32.min(c_data.len())]);
                    let mut areas = Vec::new();

                    if c_data.len() >= 0x12c + 16 {
                        let num_points =
                            u32::from_be_bytes(c_data[0x124..0x128].try_into().unwrap()) as usize;
                        let area_id = u32::from_be_bytes(c_data[0x128..0x12c].try_into().unwrap());

                        let num_pts = num_points.max(2).min(32);
                        let target_off = 0x12c + num_pts * 8;

                        let mut min_x = i32::MAX;
                        let mut min_y = i32::MAX;
                        let mut max_x = i32::MIN;
                        let mut max_y = i32::MIN;

                        for i in 0..num_pts {
                            let p_off = 0x12c + i * 8;
                            if p_off + 8 <= c_data.len() {
                                let x = i32::from_be_bytes(c_data[p_off..p_off + 4].try_into().unwrap());
                                let y = i32::from_be_bytes(c_data[p_off + 4..p_off + 8].try_into().unwrap());
                                min_x = min_x.min(x);
                                max_x = max_x.max(x);
                                min_y = min_y.min(y);
                                max_y = max_y.max(y);
                            }
                        }

                        if min_x == i32::MAX {
                            min_x = 0;
                            max_x = 0;
                            min_y = 0;
                            max_y = 0;
                        }

                        let raw_target = if target_off < c_data.len() {
                            extract_null_terminated_str(&c_data[target_off..])
                        } else {
                            String::new()
                        };
                        let target = raw_target
                            .trim()
                            .trim_matches(|c: char| c.is_control() || c == '\0')
                            .to_string();

                        // Check offset 100 (0x64) for VCDSCRIPT target line number (e.g. "100", "150", "200")
                        let script_entry_line = if c_data.len() > 100 {
                            let s = extract_null_terminated_str(&c_data[100..]);
                            s.parse::<u32>().ok()
                        } else {
                            None
                        };

                        areas.push(MapArea {
                            area_id,
                            x1: min_x,
                            y1: min_y,
                            x2: max_x,
                            y2: max_y,
                            target,
                            script_entry_line,
                        });
                    }

                    ChunkPayload::MapHotspots { map_name, areas }
                }
                CHUNK_TYPE_BGSOUND => {
                    let mut snd = String::new();
                    for try_off in [0x00, 0x08, 0x10, 0x20] {
                        if try_off < c_data.len() {
                            let s = extract_null_terminated_str(&c_data[try_off..]);
                            if s.to_uppercase().ends_with(".WAV") {
                                snd = s;
                                break;
                            }
                        }
                    }
                    if snd.is_empty() {
                        snd = extract_null_terminated_str(c_data);
                    }
                    ChunkPayload::BgSound(snd)
                }
                CHUNK_TYPE_VCDSCRIPT => {
                    // In VCDSCRIPT chunk:
                    // 0x00..0x0A: "VCDSCRIPT\0"
                    // 0x14..0x18: 32-bit big endian script text length
                    // 0x18..: plain text script source
                    if c_data.len() > 0x18 && c_data.starts_with(b"VCDSCRIPT") {
                        let text_len =
                            u32::from_be_bytes(c_data[0x14..0x18].try_into().unwrap()) as usize;
                        let text_bytes = if 0x18 + text_len <= c_data.len() {
                            &c_data[0x18..0x18 + text_len]
                        } else {
                            &c_data[0x18..]
                        };
                        let script_code = String::from_utf8_lossy(text_bytes).to_string();
                        ChunkPayload::VcdScript(script_code)
                    } else {
                        let s = String::from_utf8_lossy(c_data).to_string();
                        ChunkPayload::VcdScript(s)
                    }
                }
                _ => ChunkPayload::Raw {
                    chunk_type: c_type,
                    flags: c_flags,
                    data: c_data.to_vec(),
                },
            };

            chunks.push(payload);
        }

        Ok(Self {
            title,
            author,
            width,
            height,
            bg_color_idx,
            palette_count,
            palette,
            chunks,
        })
    }

    /// Finds the main background image filename from IMG chunks.
    pub fn get_background_image(&self) -> Option<&str> {
        for chunk in &self.chunks {
            if let ChunkPayload::Image(img) = chunk {
                if !img.filename.is_empty() {
                    return Some(&img.filename);
                }
            }
        }
        None
    }

    /// Finds any background sound from BGSOUND chunks.
    pub fn get_background_sound(&self) -> Option<&str> {
        for chunk in &self.chunks {
            if let ChunkPayload::BgSound(snd) = chunk {
                if !snd.is_empty() {
                    return Some(snd);
                }
            }
        }
        None
    }

    /// Finds any embedded VCDSCRIPT code.
    pub fn get_script(&self) -> Option<&str> {
        for chunk in &self.chunks {
            if let ChunkPayload::VcdScript(script) = chunk {
                return Some(script);
            }
        }
        None
    }

    /// Collects all map hotspot areas.
    pub fn get_all_hotspots(&self) -> Vec<&MapArea> {
        let mut res = Vec::new();
        for chunk in &self.chunks {
            if let ChunkPayload::MapHotspots { areas, .. } = chunk {
                for a in areas {
                    res.push(a);
                }
            }
        }
        res
    }
}
