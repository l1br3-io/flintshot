//! Minimal in-house ISO-BMFF (MP4) box walker.
//!
//! Just enough to answer "is this file a plausible finalized MP4":
//! walk the top-level boxes (`ftyp`, `mdat`, `moov`, …) and report
//! what exists. Deliberately no ffprobe/mediainfo dependency — the
//! finalize contract is `moov` present after stop, absent before,
//! and that is a 8-byte-header walk, not a demux.

/// One top-level box: its 4-byte type, byte offset, and total size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopLevelBox {
    /// The box type as 4 raw bytes (`b"moov"`).
    pub kind: [u8; 4],
    /// Byte offset of the box header within the file.
    pub offset: u64,
    /// Total box size in bytes, header included.
    pub size: u64,
}

impl TopLevelBox {
    /// Whether this box is of the given 4-byte type.
    #[must_use]
    pub fn is_kind(&self, kind: &[u8; 4]) -> bool {
        &self.kind == kind
    }
}

/// Walk the top-level boxes of `bytes`. Stops at the first
/// malformed/truncated header; a zero size means "box extends to
/// end of file" and ends the walk.
#[must_use]
pub fn top_level_boxes(bytes: &[u8]) -> Vec<TopLevelBox> {
    let mut boxes = Vec::new();
    let total = bytes.len() as u64;
    let mut offset: u64 = 0;

    while offset + 8 <= total {
        let Ok(start) = usize::try_from(offset) else {
            break;
        };
        let size32 = u64::from(u32::from_be_bytes([
            bytes[start],
            bytes[start + 1],
            bytes[start + 2],
            bytes[start + 3],
        ]));
        let kind = [
            bytes[start + 4],
            bytes[start + 5],
            bytes[start + 6],
            bytes[start + 7],
        ];

        let (header_size, size, to_eof) = if size32 == 1 {
            // size==1: the real 64-bit size follows the type.
            let Some(large): Option<[u8; 8]> = bytes
                .get(start + 8..start + 16)
                .and_then(|s| s.try_into().ok())
            else {
                break;
            };
            (16u64, u64::from_be_bytes(large), false)
        } else if size32 == 0 {
            // size==0: box extends through end of file.
            (8u64, total - offset, true)
        } else {
            (8u64, size32, false)
        };

        if size < header_size || offset + size > total {
            // Truncated or malformed: stop scanning rather than guess.
            break;
        }
        boxes.push(TopLevelBox { kind, offset, size });
        if to_eof {
            break;
        }
        offset += size;
    }
    boxes
}

/// Whether a top-level box of `kind` (4 bytes) exists in `bytes`.
#[must_use]
pub fn has_box(bytes: &[u8], kind: &[u8; 4]) -> bool {
    top_level_boxes(bytes).iter().any(|b| b.is_kind(kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a box: big-endian u32 size, 4-byte type, then payload.
    fn box_bytes(kind: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let size = u32::try_from(8 + payload.len()).expect("test payload < 4 GiB");
        let mut out = size.to_be_bytes().to_vec();
        out.extend_from_slice(&kind);
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn walks_ftyp_mdat_moov_in_order() {
        let ftyp = box_bytes(*b"ftyp", &[0x0a; 12]);
        let mdat = box_bytes(*b"mdat", &[0x55; 30]);
        let moov = box_bytes(*b"moov", &[0x77; 8]);
        let file = [ftyp.clone(), mdat.clone(), moov.clone()].concat();

        let boxes = top_level_boxes(&file);
        assert_eq!(boxes.len(), 3);
        assert!(boxes[0].is_kind(b"ftyp"));
        assert!(boxes[1].is_kind(b"mdat"));
        assert!(boxes[2].is_kind(b"moov"));
        assert_eq!(boxes[0].offset, 0);
        assert_eq!(boxes[1].offset, ftyp.len() as u64);
        assert_eq!(boxes[2].offset, (ftyp.len() + mdat.len()) as u64);
        assert_eq!(boxes[0].size, ftyp.len() as u64);
        assert_eq!(boxes[2].size, moov.len() as u64);
    }

    #[test]
    fn empty_and_tiny_inputs_yield_no_boxes() {
        assert!(top_level_boxes(&[]).is_empty());
        assert!(top_level_boxes(&[0, 0, 0]).is_empty()); // < 8 bytes: no header
    }

    #[test]
    fn truncated_box_is_skipped_not_paniced_on() {
        // Header claims 64 bytes but only 20 exist.
        let mut file = 64u32.to_be_bytes().to_vec();
        file.extend_from_slice(b"mdat");
        file.extend_from_slice(&[0u8; 12]);
        assert!(top_level_boxes(&file).is_empty());
    }

    #[test]
    fn zero_size_box_extends_to_end_of_file() {
        // A final box with size 0 means "rest of file": one entry,
        // walk ends.
        let mut file = box_bytes(*b"ftyp", &[0x0a; 8]);
        file.extend_from_slice(&0u32.to_be_bytes());
        file.extend_from_slice(b"mdat");
        file.extend_from_slice(&[0x11; 10]);

        let boxes = top_level_boxes(&file);
        assert_eq!(boxes.len(), 2);
        assert!(boxes[1].is_kind(b"mdat"));
        // Size covers through EOF (8-byte header + 10 payload).
        assert_eq!(boxes[1].size, 18);
    }

    #[test]
    fn largesize_header_is_honored() {
        // size==1: real size follows as u64.
        let payload = [0x22u8; 16];
        let mut file = 1u32.to_be_bytes().to_vec();
        file.extend_from_slice(b"mdat");
        file.extend_from_slice(&((16 + 16) as u64).to_be_bytes());
        file.extend_from_slice(&payload);

        let boxes = top_level_boxes(&file);
        assert_eq!(boxes.len(), 1);
        assert!(boxes[0].is_kind(b"mdat"));
        assert_eq!(boxes[0].size, 32);
    }

    #[test]
    fn has_box_finds_and_misses() {
        let file = [box_bytes(*b"ftyp", &[1; 8]), box_bytes(*b"moov", &[2; 8])].concat();
        assert!(has_box(&file, b"ftyp"));
        assert!(has_box(&file, b"moov"));
        assert!(!has_box(&file, b"mdat"));
    }
}
