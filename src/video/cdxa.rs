//! CD-XA Sector Demuxer
//!
//! VideoCD 2.0 / 3.0 stores MPEG-1 Program Streams inside `MPEGAV/*.DAT` files.
//! These files are encapsulated in a RIFF container with a `CDXA` format header,
//! containing raw CD-ROM Mode 2 Form 2 sectors (2352 bytes each).
//!
//! Sector layout (2352 bytes):
//! - 0..12:   12 bytes sync pattern (00 FF FF FF FF FF FF FF FF FF FF 00)
//! - 12..16:  4 bytes sector header (Minute, Second, Sector/Frame, Mode=0x02)
//! - 16..24:  8 bytes subheader (File, Channel, Submode, Coding Info, repeated)
//! - 24..2348: 2324 bytes user data payload (MPEG-1 Program Stream packets)
//! - 2348..2352: 4 bytes EDC checksum / spare
//!
//! This module strips the 24-byte headers and 4-byte trailers from each sector
//! and trims leading zero-padding sectors to produce a clean MPEG-1 Program Stream.

/// Extracts a clean MPEG-1 Program Stream from raw RIFF/CD-XA `.DAT` data.
///
/// If the input does not have a RIFF/CDXA header (e.g. raw `.mpg` or already demuxed stream),
/// this function safely falls back to passthrough mode and returns the input data directly.
pub fn extract_mpeg_ps(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 44 {
        return Ok(data.to_vec());
    }

    // Check for "RIFF" and "CDXA"
    let is_riff_cdxa = &data[0..4] == b"RIFF" && &data[8..12] == b"CDXA";
    if !is_riff_cdxa {
        // Passthrough for standard MPEG-1 Program Streams
        return Ok(data.to_vec());
    }

    // Locate the 'data' chunk
    let mut data_start = None;
    for i in 12..data.len().saturating_sub(8).min(256) {
        if &data[i..i + 4] == b"data" {
            // Found 'data' chunk. Length is at i+4..i+8 (little endian), payload starts at i+8
            data_start = Some(i + 8);
            break;
        }
    }

    let payload_offset = match data_start {
        Some(offset) => offset,
        None => {
            // Fallback: standard RIFF CDXA header length is 44 bytes
            44
        }
    };

    if payload_offset >= data.len() {
        return Err("RIFF/CDXA file contains empty data chunk".to_string());
    }

    let sector_data = &data[payload_offset..];
    const SECTOR_SIZE: usize = 2352;
    const HEADER_SIZE: usize = 24;
    const PAYLOAD_SIZE: usize = 2324;

    let num_sectors = sector_data.len() / SECTOR_SIZE;
    let mut ps_stream = Vec::with_capacity(num_sectors * PAYLOAD_SIZE);

    for s in 0..num_sectors {
        let sec = &sector_data[s * SECTOR_SIZE..(s + 1) * SECTOR_SIZE];
        let payload = &sec[HEADER_SIZE..HEADER_SIZE + PAYLOAD_SIZE];
        ps_stream.extend_from_slice(payload);
    }

    // Handle any trailing partial sector
    let remainder_offset = num_sectors * SECTOR_SIZE;
    if remainder_offset + HEADER_SIZE < sector_data.len() {
        let partial_sec = &sector_data[remainder_offset..];
        let available_payload = (partial_sec.len() - HEADER_SIZE).min(PAYLOAD_SIZE);
        ps_stream.extend_from_slice(&partial_sec[HEADER_SIZE..HEADER_SIZE + available_payload]);
    }

    // Trim leading zero-padding / silence sectors by finding the first MPEG start code
    // MPEG-1 Pack Header: 00 00 01 BA
    if let Some(pos) = find_mpeg_start_code(&ps_stream) {
        if pos > 0 {
            ps_stream.drain(0..pos);
        }
    }

    Ok(ps_stream)
}

/// Finds the index of the first valid MPEG-1 start code in the stream.
///
/// Looks for:
/// - 00 00 01 BA (Pack Header - standard MPEG-1 Program Stream start)
/// - 00 00 01 B3 (Sequence Header - standard video elementary stream start)
fn find_mpeg_start_code(data: &[u8]) -> Option<usize> {
    if data.len() < 4 {
        return None;
    }

    for i in 0..data.len() - 4 {
        if data[i] == 0x00 && data[i + 1] == 0x00 && data[i + 2] == 0x01 {
            let code = data[i + 3];
            // 0xBA: Pack Header, 0xBB: System Header, 0xB3: Video Sequence Header
            if code == 0xBA || code == 0xBB || code == 0xB3 {
                return Some(i);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passthrough_non_riff() {
        let raw_mpg = vec![0x00, 0x00, 0x01, 0xba, 0x21, 0x00, 0x01, 0x02];
        let extracted = extract_mpeg_ps(&raw_mpg).expect("Passthrough failed");
        assert_eq!(extracted, raw_mpg);
    }

    #[test]
    fn test_mock_cdxa_demux() {
        // Build mock RIFF CDXA header
        let mut mock_dat = Vec::new();
        mock_dat.extend_from_slice(b"RIFF");
        mock_dat.extend_from_slice(&(2352u32 * 2 + 36).to_le_bytes());
        mock_dat.extend_from_slice(b"CDXAfmt ");
        mock_dat.extend_from_slice(&[0u8; 16]); // fmt chunk
        mock_dat.extend_from_slice(b"data");
        mock_dat.extend_from_slice(&(2352u32 * 2).to_le_bytes()); // 2 sectors

        // Sector 0: Lead-in padding sector (all zeros)
        let mut sec0 = vec![0u8; 2352];
        sec0[0..12].copy_from_slice(&[0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00]);
        mock_dat.extend_from_slice(&sec0);

        // Sector 1: Valid MPEG Pack header
        let mut sec1 = vec![0u8; 2352];
        sec1[0..12].copy_from_slice(&[0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00]);
        sec1[18] = 0x62; // submode
        // Put MPEG Pack header in payload (offset 24)
        sec1[24..28].copy_from_slice(&[0x00, 0x00, 0x01, 0xba]);
        sec1[28..32].copy_from_slice(&[0x11, 0x22, 0x33, 0x44]);
        mock_dat.extend_from_slice(&sec1);

        let extracted = extract_mpeg_ps(&mock_dat).expect("Demux failed");
        assert!(!extracted.is_empty());
        // Leading padding sector should be trimmed, first 4 bytes should be 00 00 01 BA
        assert_eq!(&extracted[0..4], &[0x00, 0x00, 0x01, 0xba]);
        assert_eq!(&extracted[4..8], &[0x11, 0x22, 0x33, 0x44]);
    }
}
