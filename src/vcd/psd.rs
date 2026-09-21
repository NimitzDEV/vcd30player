//! VCD 2.0 Play Sequence Descriptor (`PSD.VCD`) parser.
//!
//! `PSD.VCD` defines the Playback Control (PBC) interactive state graph.
//! It consists of three descriptor types:
//! - `PlayList` (0x10): Linear sequential playback of tracks or segments.
//! - `SelectionList` (0x18): Interactive menu displaying a still frame or video and awaiting user selection.
//! - `EndList` (0x1F): Marks playback termination or prompt for next disc.

use std::collections::HashMap;
use std::path::Path;

use super::lot::LotTable;

/// PlayList Descriptor (0x10)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayListDesc {
    pub byte_offset: usize,
    pub unit_offset: u16,
    pub noi: u8,
    pub lid: u16,
    pub prev_ofs: u16,
    pub next_ofs: u16,
    pub return_ofs: u16,
    /// Playing time in 1/15th second units (0 = play entire item)
    pub ptime: u16,
    /// Wait time after playback in seconds (0 = immediate next, 255 = pause indefinitely)
    pub wtime: u8,
    /// Auto pause time
    pub atime: u8,
    /// Item numbers (2..99 = MPEG track, 1000..9999 = segment item /SEGMENT/ITEMxxxx.DAT)
    pub items: Vec<u16>,
}

/// SelectionList Descriptor (0x18 / 0x1A / 0x11)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionListDesc {
    pub byte_offset: usize,
    pub unit_offset: u16,
    pub flags: u8,
    pub nos: u8,
    pub bsn: u8,
    pub lid: u16,
    pub prev_ofs: u16,
    pub next_ofs: u16,
    pub return_ofs: u16,
    pub default_ofs: u16,
    pub timeout_ofs: u16,
    /// Timeout duration in seconds (0 = infinite / loop)
    pub timeout_time: u8,
    pub loop_count: u8,
    /// Background multimedia item (2..99 = MPEG track, 1000..9999 = segment item /SEGMENT/ITEMxxxx.DAT)
    pub item_id: u16,
    /// Target offsets (in 8-byte units) for selections 0..nos-1
    pub selections: Vec<u16>,
    /// Descriptor tag (0x18 = Standard, 0x1A = Extended, 0x11 = Basic)
    pub descriptor_tag: u8,
    /// Extended area and button attributes (present if 0x1A, empty otherwise)
    pub ext_area_data: Vec<u8>,
}

/// A rectangular selection area (hotspot) on an extended selection list screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PbcSelectionArea {
    /// 0-based selection index (0..nos-1).
    pub selection_index: usize,
    /// Selection key number (bsn + selection_index).
    pub selection_number: u8,
    /// Normalized coordinates on 0..=255 grid.
    pub x1_norm: u8,
    pub y1_norm: u8,
    pub x2_norm: u8,
    pub y2_norm: u8,
    /// Pixel coordinates on standard 352x288 canvas.
    pub x1: u32,
    pub y1: u32,
    pub x2: u32,
    pub y2: u32,
}

impl PbcSelectionArea {
    /// Returns true if this selection area contains the point (px, py) on the 352x288 canvas.
    pub fn contains_pixel(&self, px: i32, py: i32) -> bool {
        if px < 0 || py < 0 {
            return false;
        }
        let px = px as u32;
        let py = py as u32;
        px >= self.x1 && px <= self.x2 && py >= self.y1 && py <= self.y2
    }

    /// Whether this selection area has a non-zero, valid bounding rectangle.
    pub fn is_valid(&self) -> bool {
        self.x2 > self.x1 && self.y2 > self.y1
    }
}

impl SelectionListDesc {
    /// Returns true if this is an Extended Selection List (0x1A).
    pub fn is_extended(&self) -> bool {
        self.descriptor_tag == 0x1A
    }

    /// Extracts all valid rectangular selection areas (hotspots) defined in an Extended Selection List (0x1A).
    pub fn get_selection_areas(&self) -> Vec<PbcSelectionArea> {
        if !self.is_extended() || self.ext_area_data.len() < 16 + (self.nos as usize) * 4 {
            return Vec::new();
        }

        let mut areas = Vec::with_capacity(self.nos as usize);
        let button_data = &self.ext_area_data[16..16 + (self.nos as usize) * 4];

        for i in 0..(self.nos as usize) {
            let offset = i * 4;
            let x1_norm = button_data[offset];
            let y1_norm = button_data[offset + 1];
            let x2_norm = button_data[offset + 2];
            let y2_norm = button_data[offset + 3];

            if x2_norm > x1_norm && y2_norm > y1_norm {
                let x1 = (x1_norm as u32 * 352) / 255;
                let y1 = (y1_norm as u32 * 288) / 255;
                let x2 = (x2_norm as u32 * 352) / 255;
                let y2 = (y2_norm as u32 * 288) / 255;

                areas.push(PbcSelectionArea {
                    selection_index: i,
                    selection_number: self.bsn.saturating_add(i as u8),
                    x1_norm,
                    y1_norm,
                    x2_norm,
                    y2_norm,
                    x1,
                    y1,
                    x2,
                    y2,
                });
            }
        }

        areas
    }
}

/// EndList Descriptor (0x1F)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndListDesc {
    pub byte_offset: usize,
    pub unit_offset: u16,
    pub next_disc: u8,
    pub change_pic: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsdDescriptor {
    PlayList(PlayListDesc),
    SelectionList(SelectionListDesc),
    EndList(EndListDesc),
}

impl PsdDescriptor {
    pub fn byte_offset(&self) -> usize {
        match self {
            PsdDescriptor::PlayList(p) => p.byte_offset,
            PsdDescriptor::SelectionList(s) => s.byte_offset,
            PsdDescriptor::EndList(e) => e.byte_offset,
        }
    }

    pub fn unit_offset(&self) -> u16 {
        match self {
            PsdDescriptor::PlayList(p) => p.unit_offset,
            PsdDescriptor::SelectionList(s) => s.unit_offset,
            PsdDescriptor::EndList(e) => e.unit_offset,
        }
    }

    pub fn lid(&self) -> Option<u16> {
        match self {
            PsdDescriptor::PlayList(p) => Some(p.lid),
            PsdDescriptor::SelectionList(s) => Some(s.lid),
            PsdDescriptor::EndList(_) => None,
        }
    }

    pub fn prev_ofs(&self) -> u16 {
        match self {
            PsdDescriptor::PlayList(p) => p.prev_ofs,
            PsdDescriptor::SelectionList(s) => s.prev_ofs,
            PsdDescriptor::EndList(_) => 0xFFFF,
        }
    }

    pub fn next_ofs(&self) -> u16 {
        match self {
            PsdDescriptor::PlayList(p) => p.next_ofs,
            PsdDescriptor::SelectionList(s) => s.next_ofs,
            PsdDescriptor::EndList(_) => 0xFFFF,
        }
    }

    pub fn return_ofs(&self) -> u16 {
        match self {
            PsdDescriptor::PlayList(p) => p.return_ofs,
            PsdDescriptor::SelectionList(s) => s.return_ofs,
            PsdDescriptor::EndList(_) => 0xFFFF,
        }
    }

    /// Returns true if this descriptor is an Extended Selection List.
    pub fn is_extended(&self) -> bool {
        match self {
            PsdDescriptor::SelectionList(s) => s.is_extended(),
            _ => false,
        }
    }
}

/// Complete parsed table of descriptors from `PSD.VCD` or `PSD_X.VCD`.
#[derive(Debug, Clone)]
pub struct PsdTable {
    pub descriptors: Vec<PsdDescriptor>,
    pub offset_map: HashMap<usize, usize>, // byte_offset -> index in descriptors
    pub unit_map: HashMap<u16, usize>,     // unit_offset -> index in descriptors
    pub offset_mult: usize,
}

impl PsdTable {
    /// Parses `PSD.VCD` or `PSD_X.VCD` from raw bytes.
    pub fn parse(data: &[u8], offset_mult: usize) -> Result<Self, String> {
        let mult = if offset_mult == 0 { 8 } else { offset_mult };
        let mut descriptors = Vec::new();
        let mut offset_map = HashMap::new();
        let mut unit_map = HashMap::new();

        let mut pos = 0;
        while pos < data.len() {
            let desc_type = data[pos];
            if desc_type == 0 {
                pos += 1;
                continue;
            }

            let byte_offset = pos;
            let unit_offset = (pos / mult) as u16;

            match desc_type {
                0x10 => {
                    // PlayList
                    if pos + 14 > data.len() {
                        break;
                    }
                    let noi = data[pos + 1] as usize;
                    let desc_len = 14 + noi * 2;
                    if pos + desc_len > data.len() {
                        break;
                    }

                    let lid = u16::from_be_bytes([data[pos + 2], data[pos + 3]]);
                    let prev_ofs = u16::from_be_bytes([data[pos + 4], data[pos + 5]]);
                    let next_ofs = u16::from_be_bytes([data[pos + 6], data[pos + 7]]);
                    let return_ofs = u16::from_be_bytes([data[pos + 8], data[pos + 9]]);
                    let ptime = u16::from_be_bytes([data[pos + 10], data[pos + 11]]);
                    let wtime = data[pos + 12];
                    let atime = data[pos + 13];

                    let mut items = Vec::with_capacity(noi);
                    for i in 0..noi {
                        let item_base = pos + 14 + i * 2;
                        let item_id = u16::from_be_bytes([data[item_base], data[item_base + 1]]);
                        items.push(item_id);
                    }

                    let desc = PlayListDesc {
                        byte_offset,
                        unit_offset,
                        noi: noi as u8,
                        lid,
                        prev_ofs,
                        next_ofs,
                        return_ofs,
                        ptime,
                        wtime,
                        atime,
                        items,
                    };

                    let idx = descriptors.len();
                    descriptors.push(PsdDescriptor::PlayList(desc));
                    offset_map.insert(byte_offset, idx);
                    unit_map.insert(unit_offset, idx);

                    let padded_len = desc_len.div_ceil(mult) * mult;
                    pos += padded_len;
                }
                0x18 | 0x1A | 0x11 => {
                    // SelectionList (0x18 = Standard, 0x1A = Extended, 0x11 = Basic)
                    if pos + 20 > data.len() {
                        break;
                    }
                    let flags = data[pos + 1];
                    let nos = data[pos + 2] as usize;
                    let bsn = data[pos + 3];

                    // Standard (0x18/0x11) has 20 + nos * 2 bytes.
                    // Extended (0x1A) adds 16 bytes general area info + nos * 4 bytes button areas.
                    let raw_len = if desc_type == 0x1A {
                        36 + nos * 6
                    } else {
                        20 + nos * 2
                    };
                    if pos + raw_len > data.len() {
                        break;
                    }

                    let lid = u16::from_be_bytes([data[pos + 4], data[pos + 5]]);
                    let prev_ofs = u16::from_be_bytes([data[pos + 6], data[pos + 7]]);
                    let next_ofs = u16::from_be_bytes([data[pos + 8], data[pos + 9]]);
                    let return_ofs = u16::from_be_bytes([data[pos + 10], data[pos + 11]]);
                    let default_ofs = u16::from_be_bytes([data[pos + 12], data[pos + 13]]);
                    let timeout_ofs = u16::from_be_bytes([data[pos + 14], data[pos + 15]]);
                    let timeout_time = data[pos + 16];
                    let loop_count = data[pos + 17];
                    let item_id = u16::from_be_bytes([data[pos + 18], data[pos + 19]]);

                    let mut selections = Vec::with_capacity(nos);
                    for i in 0..nos {
                        let sel_base = pos + 20 + i * 2;
                        let sel_ofs = u16::from_be_bytes([data[sel_base], data[sel_base + 1]]);
                        selections.push(sel_ofs);
                    }

                    let ext_area_data = if desc_type == 0x1A {
                        data[pos + 20 + nos * 2..pos + raw_len].to_vec()
                    } else {
                        Vec::new()
                    };

                    let desc = SelectionListDesc {
                        byte_offset,
                        unit_offset,
                        flags,
                        nos: nos as u8,
                        bsn,
                        lid,
                        prev_ofs,
                        next_ofs,
                        return_ofs,
                        default_ofs,
                        timeout_ofs,
                        timeout_time,
                        loop_count,
                        item_id,
                        selections,
                        descriptor_tag: desc_type,
                        ext_area_data,
                    };

                    let idx = descriptors.len();
                    descriptors.push(PsdDescriptor::SelectionList(desc));
                    offset_map.insert(byte_offset, idx);
                    unit_map.insert(unit_offset, idx);

                    let padded_len = raw_len.div_ceil(mult) * mult;
                    pos += padded_len;
                }
                0x1F => {
                    // EndList
                    if pos + 4 > data.len() {
                        break;
                    }
                    let next_disc = data[pos + 1];
                    let change_pic = u16::from_be_bytes([data[pos + 2], data[pos + 3]]);

                    let desc = EndListDesc {
                        byte_offset,
                        unit_offset,
                        next_disc,
                        change_pic,
                    };

                    let idx = descriptors.len();
                    descriptors.push(PsdDescriptor::EndList(desc));
                    offset_map.insert(byte_offset, idx);
                    unit_map.insert(unit_offset, idx);
                    let padded_len = 8_usize.div_ceil(mult) * mult;
                    pos += padded_len;
                }
                _ => {
                    // Unknown or padding
                    pos += 1;
                }
            }
        }

        Ok(Self {
            descriptors,
            offset_map,
            unit_map,
            offset_mult: mult,
        })
    }

    /// Loads and parses `PSD.VCD` from file path.
    pub fn from_file<P: AsRef<Path>>(path: P, offset_mult: usize) -> Result<Self, String> {
        let bytes = std::fs::read(path.as_ref())
            .map_err(|e| format!("Failed to read PSD file: {}", e))?;
        Self::parse(&bytes, offset_mult)
    }

    /// Look up descriptor by exact unit offset (in units of `offset_mult` bytes).
    pub fn get_by_unit_offset(&self, unit_ofs: u16) -> Option<&PsdDescriptor> {
        if unit_ofs == 0xFFFF {
            return None;
        }
        self.unit_map.get(&unit_ofs).map(|&idx| &self.descriptors[idx])
    }

    /// Look up descriptor by byte offset.
    pub fn get_by_byte_offset(&self, byte_ofs: usize) -> Option<&PsdDescriptor> {
        self.offset_map.get(&byte_ofs).map(|&idx| &self.descriptors[idx])
    }

    /// Look up descriptor by List ID using `LOT.VCD`.
    pub fn get_by_lid(&self, lid: u16, lot: &LotTable) -> Option<&PsdDescriptor> {
        let unit_ofs = lot.get_unit_offset(lid)?;
        self.get_by_unit_offset(unit_ofs)
    }

    /// Returns the first `SelectionList` on this disc (representing the root/primary interactive menu).
    pub fn first_selection_list(&self) -> Option<&SelectionListDesc> {
        for desc in &self.descriptors {
            if let PsdDescriptor::SelectionList(sel) = desc {
                return Some(sel);
            }
        }
        None
    }

    /// Returns true if any descriptor in this table is an Extended Selection List.
    pub fn has_extended_descriptors(&self) -> bool {
        self.descriptors.iter().any(|d| d.is_extended())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psd_parse_synthetic_playlist_and_selection() {
        let mut raw = Vec::new();
        // 1. PlayList at 0x0000 (unit 0)
        raw.push(0x10); // desc_type
        raw.push(1);    // noi = 1
        raw.extend_from_slice(&1u16.to_be_bytes()); // lid = 1
        raw.extend_from_slice(&0xFFFFu16.to_be_bytes()); // prev_ofs
        raw.extend_from_slice(&2u16.to_be_bytes()); // next_ofs = 2 (unit offset)
        raw.extend_from_slice(&0xFFFFu16.to_be_bytes()); // return_ofs
        raw.extend_from_slice(&0u16.to_be_bytes()); // ptime = 0
        raw.push(0); // wtime = 0
        raw.push(0); // atime = 0
        raw.extend_from_slice(&19u16.to_be_bytes()); // items[0] = 19
        assert_eq!(raw.len(), 16);

        // 2. SelectionList at 0x0010 (unit 2)
        raw.push(0x18); // desc_type
        raw.push(0x00); // flags
        raw.push(2);    // nos = 2
        raw.push(1);    // bsn = 1
        raw.extend_from_slice(&2u16.to_be_bytes()); // lid = 2
        raw.extend_from_slice(&0u16.to_be_bytes()); // prev_ofs = 0
        raw.extend_from_slice(&9u16.to_be_bytes()); // next_ofs = 9
        raw.extend_from_slice(&0u16.to_be_bytes()); // return_ofs = 0
        raw.extend_from_slice(&0xFFFFu16.to_be_bytes()); // default_ofs
        raw.extend_from_slice(&9u16.to_be_bytes()); // timeout_ofs = 9
        raw.push(15); // timeout_time = 15s
        raw.push(1);  // loop = 1
        raw.extend_from_slice(&18u16.to_be_bytes()); // item_id = 18 (motion menu track)
        raw.extend_from_slice(&9u16.to_be_bytes());  // sel 0 -> unit 9
        raw.extend_from_slice(&11u16.to_be_bytes()); // sel 1 -> unit 11
        assert_eq!(raw.len(), 16 + 24);

        // 3. EndList at 0x0028 (unit 5)
        raw.push(0x1F); // desc_type
        raw.push(0);    // next_disc
        raw.extend_from_slice(&0u16.to_be_bytes()); // change_pic
        raw.extend_from_slice(&[0, 0, 0, 0]); // padding to 8 bytes

        let psd = PsdTable::parse(&raw, 8).expect("failed to parse psd");
        assert_eq!(psd.descriptors.len(), 3);
        assert!(!psd.has_extended_descriptors());

        // Check PlayList
        match psd.get_by_unit_offset(0) {
            Some(PsdDescriptor::PlayList(p)) => {
                assert_eq!(p.lid, 1);
                assert_eq!(p.items, vec![19]);
                assert_eq!(p.next_ofs, 2);
            }
            _ => panic!("expected PlayList at unit 0"),
        }

        // Check SelectionList
        match psd.get_by_unit_offset(2) {
            Some(PsdDescriptor::SelectionList(s)) => {
                assert_eq!(s.lid, 2);
                assert_eq!(s.nos, 2);
                assert_eq!(s.bsn, 1);
                assert_eq!(s.item_id, 18);
                assert_eq!(s.selections, vec![9, 11]);
                assert_eq!(s.timeout_time, 15);
                assert!(!s.is_extended());
            }
            _ => panic!("expected SelectionList at unit 2"),
        }

        // Check EndList
        match psd.get_by_unit_offset(5) {
            Some(PsdDescriptor::EndList(e)) => {
                assert_eq!(e.next_disc, 0);
            }
            _ => panic!("expected EndList at unit 5"),
        }

        let first_sel = psd.first_selection_list().expect("first selection list");
        assert_eq!(first_sel.lid, 2);
    }

    #[test]
    fn test_psd_parse_extended_selection_list() {
        let mut raw = Vec::new();
        // Extended SelectionList (0x1A) at unit 0
        raw.push(0x1A); // desc_type
        raw.push(0x00); // flags
        raw.push(2);    // nos = 2
        raw.push(1);    // bsn = 1
        raw.extend_from_slice(&10u16.to_be_bytes()); // lid = 10
        raw.extend_from_slice(&0u16.to_be_bytes());  // prev_ofs = 0
        raw.extend_from_slice(&19u16.to_be_bytes()); // next_ofs = 19
        raw.extend_from_slice(&0u16.to_be_bytes());  // return_ofs = 0
        raw.extend_from_slice(&0xFFFFu16.to_be_bytes()); // default_ofs
        raw.extend_from_slice(&19u16.to_be_bytes()); // timeout_ofs = 19
        raw.push(0); // timeout_time = 0
        raw.push(1); // loop = 1
        raw.extend_from_slice(&18u16.to_be_bytes()); // item_id = 18
        // Selections: 2 * 2 = 4 bytes
        raw.extend_from_slice(&19u16.to_be_bytes()); // sel 0 -> unit 19
        raw.extend_from_slice(&21u16.to_be_bytes()); // sel 1 -> unit 21
        // Extended area info: 16 bytes
        raw.extend_from_slice(&[0x01; 16]);
        // Button coordinates: nos * 4 = 8 bytes
        raw.extend_from_slice(&[0x02; 8]);
        // Raw length so far: 20 + 4 + 16 + 8 = 48 bytes (divisible by 8)
        assert_eq!(raw.len(), 48);

        // Next descriptor: PlayList at unit 6 (byte 48)
        raw.push(0x10);
        raw.push(1); // noi = 1
        raw.extend_from_slice(&11u16.to_be_bytes()); // lid = 11
        raw.extend_from_slice(&0xFFFFu16.to_be_bytes());
        raw.extend_from_slice(&0xFFFFu16.to_be_bytes());
        raw.extend_from_slice(&0xFFFFu16.to_be_bytes());
        raw.extend_from_slice(&0u16.to_be_bytes());
        raw.push(0);
        raw.push(0);
        raw.extend_from_slice(&2u16.to_be_bytes()); // items[0] = 2

        let psd = PsdTable::parse(&raw, 8).expect("failed to parse extended psd");
        assert_eq!(psd.descriptors.len(), 2);
        assert!(psd.has_extended_descriptors());

        match psd.get_by_unit_offset(0) {
            Some(PsdDescriptor::SelectionList(s)) => {
                assert_eq!(s.lid, 10);
                assert_eq!(s.descriptor_tag, 0x1A);
                assert!(s.is_extended());
                assert_eq!(s.selections, vec![19, 21]);
                assert_eq!(s.ext_area_data.len(), 24); // 16 + 8
                assert_eq!(&s.ext_area_data[0..16], &[0x01; 16]);
                assert_eq!(&s.ext_area_data[16..24], &[0x02; 8]);
            }
            _ => panic!("expected Extended SelectionList at unit 0"),
        }

        match psd.get_by_unit_offset(6) {
            Some(PsdDescriptor::PlayList(p)) => {
                assert_eq!(p.lid, 11);
                assert_eq!(p.items, vec![2]);
            }
            _ => panic!("expected PlayList at unit 6"),
        }
    }
}
