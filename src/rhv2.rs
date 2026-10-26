//! Clean-Room RHV2 Planar Band Encoder for Epson ESC/P-R Level 5 (`bsn2`, `CompMode = 1`).
//!
//! Implemented strictly from the mathematical bitstream specification in `docs/rhv2-spec.md`.
//!
//! SPDX-License-Identifier: GPL-3.0-or-later

/// Pre-computed RHV2 payload for an all-white (`255, 255, 255`) band of any width and height.
pub const ALL_WHITE_RHV2_PAYLOAD: [u8; 48] = [
    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02,
    0xFB, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFB, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFB, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xD7, 0x1F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const fn build_sym_tables() -> ([u16; 256], [u8; 256]) {
    let mut code = [0u16; 256];
    let mut bits = [0u8; 256];

    code[0] = 0x05;
    bits[0] = 3;
    code[1] = 0x04;
    bits[1] = 3;
    code[255] = 0x00;
    bits[255] = 3;
    code[128] = 0x3F;
    bits[128] = 7;

    let mut k = 2usize;
    while k < 4 {
        code[k] = ((k as u16) << 2) | 0x02;
        bits[k] = 4;
        code[256 - k] = (((3 - k) as u16) << 2) | 0x02;
        bits[256 - k] = 4;
        k += 1;
    }

    k = 4;
    while k < 8 {
        code[k] = ((k as u16) << 3) | 0x01;
        bits[k] = 6;
        code[256 - k] = (((7 - k) as u16) << 3) | 0x01;
        bits[256 - k] = 6;
        k += 1;
    }

    k = 8;
    while k < 16 {
        code[k] = ((k as u16) << 3) | 0x03;
        bits[k] = 7;
        code[256 - k] = (((15 - k) as u16) << 3) | 0x03;
        bits[256 - k] = 7;
        k += 1;
    }

    k = 16;
    while k < 32 {
        code[k] = ((k as u16) << 4) | 0x07;
        bits[k] = 9;
        code[256 - k] = (((31 - k) as u16) << 4) | 0x07;
        bits[256 - k] = 9;
        k += 1;
    }

    k = 32;
    while k < 64 {
        code[k] = ((k as u16) << 5) | 0x0F;
        bits[k] = 11;
        code[256 - k] = (((63 - k) as u16) << 5) | 0x0F;
        bits[256 - k] = 11;
        k += 1;
    }

    k = 64;
    while k < 128 {
        code[k] = ((k as u16) << 6) | 0x1F;
        bits[k] = 13;
        code[256 - k] = (((127 - k) as u16) << 6) | 0x1F;
        bits[256 - k] = 13;
        k += 1;
    }

    (code, bits)
}

const SYM_TABLES: ([u16; 256], [u8; 256]) = build_sym_tables();

struct BitWriter {
    out: Vec<u8>,
    acc: u64,
    count: u32,
}

impl BitWriter {
    fn with_capacity(cap: usize) -> Self {
        Self {
            out: Vec::with_capacity(cap),
            acc: 0,
            count: 0,
        }
    }

    #[inline]
    fn put_bits(&mut self, val: u16, nbits: u8) {
        self.acc |= u64::from(val) << self.count;
        self.count += u32::from(nbits);
        while self.count >= 8 {
            self.out.push((self.acc & 0xFF) as u8);
            self.acc >>= 8;
            self.count -= 8;
        }
    }

    #[inline]
    fn put_sym(&mut self, s: u8) {
        let idx = usize::from(s);
        self.put_bits(SYM_TABLES.0[idx], SYM_TABLES.1[idx]);
    }

    #[inline]
    fn emit_rle(&mut self, r: usize, end_pos: usize, n: usize) {
        let mut rem = if end_pos == n { 0 } else { r };
        loop {
            let chunk = rem.min(255);
            let s = if chunk == 0 {
                128u8
            } else if (chunk & 1) != 0 {
                (chunk >> 1) as u8
            } else {
                (256 - (chunk >> 1)) as u8
            };
            self.put_sym(s);
            if rem <= 254 {
                break;
            }
            rem -= 255;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.count > 0 {
            self.out.push((self.acc & 0xFF) as u8);
        }
        self.out
    }
}

/// Compress a single byte plane according to Section 4 of `docs/rhv2-spec.md`.
pub fn compress_plane(plane: &[u8]) -> Vec<u8> {
    let n = plane.len();
    if n == 0 {
        return Vec::new();
    }

    let mut bw = BitWriter::with_capacity(n / 2 + 16);
    let mut last_run: usize = 1;
    let mut repeat_run: usize = 1;
    let mut alt_val: u8 = 0;
    let mut last_val: u8 = 255;
    let mut mode: u8 = 2;
    let mut pos: usize = 0;

    while pos < n && bw.out.len() <= n {
        let v = plane[pos];

        // Case A: Run of last_val
        if v == last_val {
            let mut end_pos = pos + 1;
            while end_pos < n && plane[end_pos] == last_val {
                end_pos += 1;
            }
            let r = end_pos - pos;
            if mode == 2 {
                bw.put_bits(0x03, 3);
            } else {
                bw.put_bits(0x07, 4);
            }
            if r == last_run {
                repeat_run = r;
            }
            bw.emit_rle(r, end_pos, n);
            last_run = r;
            pos = end_pos;
            continue;
        }

        // Case B: Run of alt_val with Large Magnitude Step
        if v == alt_val {
            let diff = i32::from(v) - i32::from(last_val);
            let mag = if diff >= 0 {
                (diff & 0xFF) as u8
            } else {
                ((-(i32::from(v) + i32::from(last_val))) & 0xFF) as u8
            };
            if (mag & 0xF8) != 0 {
                let mut end_pos = pos + 1;
                while end_pos < n && plane[end_pos] == v {
                    end_pos += 1;
                }
                let r = end_pos - pos;
                if mode == 2 {
                    bw.put_bits(0x01, 2);
                } else {
                    bw.put_bits(0x0F, 5);
                }
                mode = 2;
                if r == last_run {
                    repeat_run = r;
                }
                bw.emit_rle(r, end_pos, n);
                last_run = r;
                alt_val = last_val;
                last_val = v;
                pos = end_pos;
                continue;
            }
        }

        // Case C: Final Pixel of Plane OR Immediate Pair/Run of New Value
        if pos + 1 == n || plane[pos + 1] == v {
            let delta = if pos == 0 {
                v
            } else {
                v.wrapping_sub(plane[pos - 1])
            };
            let (mut r, end_pos) = if pos + 1 == n {
                (0usize, n)
            } else {
                let mut ep = pos + 2;
                while ep < n && plane[ep] == v {
                    ep += 1;
                }
                (ep - (pos + 1), ep)
            };

            if mode == 4 && r == repeat_run {
                bw.put_bits(0x00, 1);
                r = 0;
                last_run = repeat_run;
            } else if mode == 2 {
                bw.put_bits(0x07, 4);
            } else {
                bw.put_bits(0x01, 2);
            }
            bw.put_sym(delta);
            mode = 4;

            if r == 0 {
                alt_val = last_val;
                last_val = v;
                pos = end_pos;
            } else {
                if r == last_run {
                    repeat_run = r;
                }
                bw.emit_rle(r, end_pos, n);
                last_run = r;
                alt_val = last_val;
                last_val = v;
                pos = end_pos;
            }
            continue;
        }

        // Case D: Literal Delta Stream Until End-of-Plane or 3 Identical Pixels
        if mode == 3 {
            bw.put_bits(0x00, 1);
        } else if mode == 2 {
            bw.put_bits(0x0F, 5);
        } else {
            bw.put_bits(0x1F, 6);
        }

        let mut k = pos;
        loop {
            let u = plane[k];
            if u != last_val {
                alt_val = last_val;
            }
            let delta = if k == 0 {
                u
            } else {
                u.wrapping_sub(plane[k - 1])
            };
            bw.put_sym(delta);
            last_val = u;

            let k_next = k + 1;
            if k_next == n {
                pos = n;
                break;
            }
            if k_next + 1 < n && plane[k_next] == u && plane[k_next + 1] == u {
                bw.put_bits(0x7F, 7);
                let mut end_pos = k_next + 2;
                while end_pos < n && plane[end_pos] == u {
                    end_pos += 1;
                }
                let r = end_pos - k_next;
                mode = 3;
                if r == last_run {
                    repeat_run = r;
                }
                bw.emit_rle(r, end_pos, n);
                last_run = r;
                last_val = u;
                pos = end_pos;
                break;
            }
            k = k_next;
        }
    }

    let comp = bw.finish();
    if comp.len() > n {
        let mut raw = Vec::with_capacity(1 + n);
        raw.push(0x7F);
        raw.extend_from_slice(plane);
        raw
    } else {
        comp
    }
}

/// Compress a 24-bit RGB band (`print_width * height * 3` bytes) into an ESC/P-R
/// Level 5 RHV2 payload (16-byte plane length header + 4 8-byte-aligned planes).
pub fn compress_rgb_band(rgb_bytes: &[u8], print_width: usize, height: usize) -> Vec<u8> {
    if rgb_bytes.iter().all(|&b| b == 0xFF) {
        return ALL_WHITE_RHV2_PAYLOAD.to_vec();
    }

    let aligned_w = (print_width + 7) & !7;
    let npixels = aligned_w * height;
    let mut p0 = vec![0xFFu8; npixels];
    let mut p1 = vec![0xFFu8; npixels];
    let mut p2 = vec![0xFFu8; npixels];
    let p3 = vec![0x00u8; npixels];

    let row_stride = print_width * 3;
    for y in 0..height {
        let src_row = &rgb_bytes[y * row_stride..(y + 1) * row_stride];
        let dst_off = y * aligned_w;
        for x in 0..print_width {
            let sx = x * 3;
            p0[dst_off + x] = src_row[sx];
            p1[dst_off + x] = src_row[sx + 1];
            p2[dst_off + x] = src_row[sx + 2];
        }
    }

    let planes = [
        compress_plane(&p0),
        compress_plane(&p1),
        compress_plane(&p2),
        compress_plane(&p3),
    ];

    let mut total_payload = 16usize;
    for cp in &planes {
        total_payload += (cp.len() + 7) & !7;
    }

    let mut out = Vec::with_capacity(total_payload);
    for cp in &planes {
        out.extend_from_slice(&(cp.len() as u32).to_be_bytes());
    }
    for cp in &planes {
        out.extend_from_slice(cp);
        let pad = ((cp.len() + 7) & !7) - cp.len();
        if pad > 0 {
            out.resize(out.len() + pad, 0);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_to_bytes(hex: &str) -> Vec<u8> {
        let clean: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
        (0..clean.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&clean[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn test_rhv2_reference_vectors() {
        // Vector 1: 8-Byte Constant White Plane & All-white RGB band
        let v1 = vec![255u8; 8];
        assert_eq!(compress_plane(&v1), vec![0xFB, 0x01]);
        let white_band = vec![255u8; 16 * 16 * 3];
        assert_eq!(
            compress_rgb_band(&white_band, 16, 16),
            ALL_WHITE_RHV2_PAYLOAD
        );

        // Vector 2: 8-Byte Mixed Literal & Run Plane
        let v2 = [0u8, 1, 2, 5, 5, 5, 200, 200];
        assert_eq!(compress_plane(&v2), hex_to_bytes("afa4ffd1137e"));

        // Vector 3: Synthetic 17 x 2 RGB Band (Tests Alignment Padding to 24 x 2)
        let mut v3_rgb = Vec::with_capacity(17 * 2 * 3);
        for i in 0..34usize {
            let r = ((7 * i) & 0xFF) as u8;
            let g = if (4..12).contains(&i) {
                128
            } else {
                ((3 * i) & 0xFF) as u8
            };
            let b = if !i.is_multiple_of(5) { 255 } else { 240 };
            v3_rgb.extend_from_slice(&[r, g, b]);
        }
        let expected_v3 = hex_to_bytes(
            "00000021000000190000000e00000002\
             af799ee7799ee7799ee7799ee79fe32f3e7c9ee7799ee7799ee7799ee7f9e1ff0f00000000000000\
             afeefeddffee1bb9bb3fe72f1eedeeeeeeeeeeeeeefe6dfe3f00000000000000\
             efdefecfd7ac66b5b46635abfd000000\
             d71f000000000000",
        );
        assert_eq!(compress_rgb_band(&v3_rgb, 17, 2), expected_v3);

        // Vector 4: Full State-Transition Plane (N = 328)
        let mut v4 = Vec::with_capacity(328);
        v4.extend_from_slice(&[255; 5]);
        v4.extend_from_slice(&[10, 10, 10]);
        v4.extend_from_slice(&[255, 255]);
        v4.extend_from_slice(&[10, 10, 10]);
        v4.extend_from_slice(&[20, 20, 20]);
        v4.extend_from_slice(&[30, 30, 30]);
        v4.extend_from_slice(&[40, 42, 45, 50, 50, 50]);
        v4.extend_from_slice(&[60, 63, 67, 70]);
        v4.extend_from_slice(&[80; 300]);
        v4.push(90);
        assert_eq!(
            compress_plane(&v4),
            hex_to_bytes("d3dbe221de14a6df543dfdc3d4439ffebfffa5a6")
        );

        // Vector 5: High-Entropy Uncompressed Fallback (N = 64)
        let v5: Vec<u8> = (0..64).map(|i| ((i * 73 + 19) & 0xFF) as u8).collect();
        let mut expected_v5 = vec![0x7Fu8];
        expected_v5.extend_from_slice(&v5);
        assert_eq!(compress_plane(&v5), expected_v5);
    }
}
