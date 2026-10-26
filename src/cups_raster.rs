//! Streaming CUPS Raster v2 / v3 Reader (`application/vnd.cups-raster`).
//!
//! Decodes scanlines on the fly so large pages (A3+, A2 at 720x720 dpi) never need
//! to be buffered in RAM at once.
//!
//! SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{self, Read};

#[derive(Debug, Clone)]
pub struct CupsPageHeader {
    pub media_type: String,
    pub width: u32,
    pub height: u32,
    pub bpp: u32,
    pub bytes_per_line: u32,
    pub color_space: u32,
}

pub struct CupsRasterReader<R: Read> {
    reader: R,
    magic_read: bool,
    little_endian: bool,
    is_v3: bool,
}

impl<R: Read> CupsRasterReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            magic_read: false,
            little_endian: true,
            is_v3: false,
        }
    }

    fn ensure_magic(&mut self) -> io::Result<bool> {
        if self.magic_read {
            return Ok(true);
        }
        let mut magic = [0u8; 4];
        match self.reader.read_exact(&mut magic) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
            Err(e) => return Err(e),
        }
        self.magic_read = true;
        match &magic {
            b"tSaR" | b"2SaR" => {
                self.little_endian = true;
                self.is_v3 = false;
            }
            b"RaSt" | b"RaS2" => {
                self.little_endian = false;
                self.is_v3 = false;
            }
            b"3SaR" => {
                self.little_endian = true;
                self.is_v3 = true;
            }
            b"RaS3" => {
                self.little_endian = false;
                self.is_v3 = true;
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Unsupported CUPS raster magic: {magic:?}"),
                ));
            }
        }
        Ok(true)
    }

    pub fn next_page(&mut self) -> io::Result<Option<CupsPageStream<'_, R>>> {
        if !self.ensure_magic()? {
            return Ok(None);
        }
        let mut hdr = vec![0u8; 1796];
        match self.reader.read_exact(&mut hdr) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }

        let media_type = c_str_from_slice(&hdr[128..192]);
        let u32_at = |idx: usize| -> u32 {
            let off = 256 + idx * 4;
            let bytes: [u8; 4] = hdr[off..off + 4].try_into().unwrap_or([0; 4]);
            if self.little_endian {
                u32::from_le_bytes(bytes)
            } else {
                u32::from_be_bytes(bytes)
            }
        };

        let width = u32_at(29);
        let height = u32_at(30);
        let bpp = u32_at(33);
        let bytes_per_line = u32_at(34);
        let color_space = u32_at(36);

        if width == 0 || height == 0 {
            return Ok(None);
        }

        let page_hdr = CupsPageHeader {
            media_type,
            width,
            height,
            bpp,
            bytes_per_line,
            color_space,
        };

        Ok(Some(CupsPageStream {
            parent: self,
            header: page_hdr,
            rows_Yielded: 0,
            repeat_remaining: 0,
            current_raw_row: Vec::new(),
        }))
    }
}

fn c_str_from_slice(slice: &[u8]) -> String {
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).trim().to_string()
}

#[allow(non_snake_case)]
pub struct CupsPageStream<'a, R: Read> {
    parent: &'a mut CupsRasterReader<R>,
    pub header: CupsPageHeader,
    rows_Yielded: u32,
    repeat_remaining: usize,
    current_raw_row: Vec<u8>,
}

impl<R: Read> CupsPageStream<'_, R> {
    fn decode_v2_plane(reader: &mut R, target_bytes: usize, unit_size: usize) -> io::Result<Vec<u8>> {
        let mut out = Vec::with_capacity(target_bytes + unit_size);
        let mut unit_buf = vec![0u8; unit_size];
        while out.len() < target_bytes {
            let mut b = [0u8; 1];
            reader.read_exact(&mut b)?;
            let n = usize::from(b[0]);
            if n <= 127 {
                let count = n + 1;
                reader.read_exact(&mut unit_buf)?;
                for _ in 0..count {
                    out.extend_from_slice(&unit_buf);
                }
            } else {
                let count = (256 - n) * unit_size;
                let start = out.len();
                out.resize(start + count, 0);
                reader.read_exact(&mut out[start..start + count])?;
            }
        }
        out.truncate(target_bytes);
        Ok(out)
    }

    /// Read the next scanline and convert it into 24-bit RGB (`width * 3` bytes).
    pub fn read_next_rgb_row(&mut self, dst_rgb: &mut Vec<u8>) -> io::Result<bool> {
        let h = &self.header;
        if self.rows_Yielded >= h.height {
            return Ok(false);
        }

        let bpl = h.bytes_per_line as usize;
        if self.parent.is_v3 {
            self.current_raw_row.resize(bpl, 0xFF);
            self.parent.reader.read_exact(&mut self.current_raw_row)?;
        } else if self.repeat_remaining == 0 {
            let mut rpt = [0u8; 1];
            self.parent.reader.read_exact(&mut rpt)?;
            self.repeat_remaining = usize::from(rpt[0]) + 1;
            let unit_size = ((h.bpp / 8).max(1)) as usize;
            self.current_raw_row = Self::decode_v2_plane(&mut self.parent.reader, bpl, unit_size)?;
        }

        if !self.parent.is_v3 {
            self.repeat_remaining = self.repeat_remaining.saturating_sub(1);
        }
        self.rows_Yielded += 1;

        let w = h.width as usize;
        dst_rgb.clear();
        dst_rgb.reserve(w * 3);

        match h.bpp {
            24 => {
                let take = (w * 3).min(self.current_raw_row.len());
                dst_rgb.extend_from_slice(&self.current_raw_row[..take]);
                if dst_rgb.len() < w * 3 {
                    dst_rgb.resize(w * 3, 0xFF);
                }
            }
            32 => {
                for px in self.current_raw_row.chunks_exact(4).take(w) {
                    dst_rgb.push(px[0]);
                    dst_rgb.push(px[1]);
                    dst_rgb.push(px[2]);
                }
                if dst_rgb.len() < w * 3 {
                    dst_rgb.resize(w * 3, 0xFF);
                }
            }
            8 => {
                for &g in self.current_raw_row.iter().take(w) {
                    let v = if h.color_space == 3 { 255 - g } else { g };
                    dst_rgb.extend_from_slice(&[v, v, v]);
                }
                if dst_rgb.len() < w * 3 {
                    dst_rgb.resize(w * 3, 0xFF);
                }
            }
            1 => {
                let invert = h.color_space == 3;
                for x in 0..w {
                    let byte_idx = x / 8;
                    let bit_mask = 0x80u8 >> (x & 7);
                    let bit_set = self
                        .current_raw_row
                        .get(byte_idx)
                        .map(|&b| (b & bit_mask) != 0)
                        .unwrap_or(false);
                    let v = if invert {
                        if bit_set { 0 } else { 255 }
                    } else if bit_set {
                        255
                    } else {
                        0
                    };
                    dst_rgb.extend_from_slice(&[v, v, v]);
                }
            }
            _ => {
                dst_rgb.resize(w * 3, 0xFF);
            }
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_cups_raster_v2_stream() {
        let mut stream = Vec::new();
        stream.extend_from_slice(b"2SaR"); // little-endian v2 magic
        let mut hdr = [0u8; 1796];
        // MediaType = "Baryta" (offset 128..192)
        hdr[128..134].copy_from_slice(b"Baryta");
        // Width = 4 px (offset 372), Height = 2 px (offset 376)
        hdr[372..376].copy_from_slice(&4u32.to_le_bytes());
        hdr[376..380].copy_from_slice(&2u32.to_le_bytes());
        // BitsPerColor = 8 (offset 384), BitsPerPixel = 24 (offset 388)
        hdr[384..388].copy_from_slice(&8u32.to_le_bytes());
        hdr[388..392].copy_from_slice(&24u32.to_le_bytes());
        // BytesPerLine = 12 (offset 392)
        hdr[392..396].copy_from_slice(&12u32.to_le_bytes());
        // ColorSpace = 1 (RGB, offset 400)
        hdr[400..404].copy_from_slice(&1u32.to_le_bytes());
        stream.extend_from_slice(&hdr);

        // Scanline repeat = 1 (meaning 2 identical rows), CUPS v2 repeat = 3 (meaning 3 + 1 = 4 identical pixels of RGB [10, 20, 30])
        stream.push(1); // row repeat count - 1 -> 2 rows
        stream.push(3); // pixel repeat: 3 + 1 = 4 pixels
        stream.extend_from_slice(&[10, 20, 30]);

        let mut reader = CupsRasterReader::new(Cursor::new(stream));
        let mut page = reader.next_page().unwrap().unwrap();
        assert_eq!(page.header.media_type, "Baryta");
        assert_eq!(page.header.width, 4);
        assert_eq!(page.header.height, 2);

        let mut row0 = Vec::new();
        let mut row1 = Vec::new();
        assert!(page.read_next_rgb_row(&mut row0).unwrap());
        assert!(page.read_next_rgb_row(&mut row1).unwrap());
        assert!(!page.read_next_rgb_row(&mut Vec::new()).unwrap());
        assert_eq!(row0, vec![10, 20, 30, 10, 20, 30, 10, 20, 30, 10, 20, 30]);
        assert_eq!(row1, row0);
    }
}

