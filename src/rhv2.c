/*
 * rhv2.c - Encoder for the ESC/P-R Level 5 (bsn2) RHV2 planar compression
 *          format specified in docs/rhv2-spec.md.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */


#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    uint16_t code[256];
    uint8_t bits[256];
    int initialized;
} rhv2_sym_table_t;

static rhv2_sym_table_t g_sym_table = { .initialized = 0 };

static void rhv2_init_table(void) {
    if (g_sym_table.initialized) {
        return;
    }

    g_sym_table.code[0] = 0x05u;
    g_sym_table.bits[0] = 3u;

    g_sym_table.code[1] = 0x04u;
    g_sym_table.bits[1] = 3u;

    g_sym_table.code[255] = 0x00u;
    g_sym_table.bits[255] = 3u;

    g_sym_table.code[128] = 0x3Fu;
    g_sym_table.bits[128] = 7u;

    for (uint32_t k = 2; k <= 3; ++k) {
        g_sym_table.code[k] = (uint16_t)((k << 2) | 0x02u);
        g_sym_table.bits[k] = 4u;
        g_sym_table.code[256u - k] = (uint16_t)(((3u - k) << 2) | 0x02u);
        g_sym_table.bits[256u - k] = 4u;
    }

    for (uint32_t k = 4; k <= 7; ++k) {
        g_sym_table.code[k] = (uint16_t)((k << 3) | 0x01u);
        g_sym_table.bits[k] = 6u;
        g_sym_table.code[256u - k] = (uint16_t)(((7u - k) << 3) | 0x01u);
        g_sym_table.bits[256u - k] = 6u;
    }

    for (uint32_t k = 8; k <= 15; ++k) {
        g_sym_table.code[k] = (uint16_t)((k << 3) | 0x03u);
        g_sym_table.bits[k] = 7u;
        g_sym_table.code[256u - k] = (uint16_t)(((15u - k) << 3) | 0x03u);
        g_sym_table.bits[256u - k] = 7u;
    }

    for (uint32_t k = 16; k <= 31; ++k) {
        g_sym_table.code[k] = (uint16_t)((k << 4) | 0x07u);
        g_sym_table.bits[k] = 9u;
        g_sym_table.code[256u - k] = (uint16_t)(((31u - k) << 4) | 0x07u);
        g_sym_table.bits[256u - k] = 9u;
    }

    for (uint32_t k = 32; k <= 63; ++k) {
        g_sym_table.code[k] = (uint16_t)((k << 5) | 0x0Fu);
        g_sym_table.bits[k] = 11u;
        g_sym_table.code[256u - k] = (uint16_t)(((63u - k) << 5) | 0x0Fu);
        g_sym_table.bits[256u - k] = 11u;
    }

    for (uint32_t k = 64; k <= 127; ++k) {
        g_sym_table.code[k] = (uint16_t)((k << 6) | 0x1Fu);
        g_sym_table.bits[k] = 13u;
        g_sym_table.code[256u - k] = (uint16_t)(((127u - k) << 6) | 0x1Fu);
        g_sym_table.bits[256u - k] = 13u;
    }

    g_sym_table.initialized = 1;
}

typedef struct {
    uint8_t *out;
    uint32_t len;
    uint32_t cap;
    uint32_t acc;
    uint32_t count;
} rhv2_bitwriter_t;

static inline void bw_put(rhv2_bitwriter_t *bw, uint32_t val, uint32_t nbits) {
    bw->acc |= (val << bw->count);
    bw->count += nbits;
    while (bw->count >= 8u) {
        if (bw->len < bw->cap) {
            bw->out[bw->len] = (uint8_t)(bw->acc & 0xFFu);
        }
        bw->len++;
        bw->acc >>= 8u;
        bw->count -= 8u;
    }
}

static inline void bw_put_sym(rhv2_bitwriter_t *bw, uint8_t sym) {
    bw_put(bw, g_sym_table.code[sym], g_sym_table.bits[sym]);
}

static inline void bw_flush(rhv2_bitwriter_t *bw) {
    if (bw->count > 0u) {
        if (bw->len < bw->cap) {
            bw->out[bw->len] = (uint8_t)(bw->acc & 0xFFu);
        }
        bw->len++;
        bw->acc = 0u;
        bw->count = 0u;
    }
}

static inline void emit_rle(
    rhv2_bitwriter_t *bw,
    uint32_t r,
    uint32_t end_pos,
    uint32_t n,
    int is_same_run,
    uint8_t next_last_val,
    uint8_t next_alt_val,
    uint32_t *last_run,
    uint32_t *repeat_run,
    uint8_t *last_val,
    uint8_t *alt_val,
    uint32_t *pos
) {
    if (is_same_run) {
        *repeat_run = r;
    }

    uint32_t rem = (end_pos == n) ? 0u : r;

    for (;;) {
        uint32_t chunk = (rem < 255u) ? rem : 255u;
        uint8_t sym;
        if (chunk == 0u) {
            sym = 128u;
        } else if ((chunk & 1u) != 0u) {
            sym = (uint8_t)(chunk >> 1u);
        } else {
            sym = (uint8_t)((256u - (chunk >> 1u)) & 0xFFu);
        }
        bw_put_sym(bw, sym);
        if (rem <= 254u) {
            break;
        }
        rem -= 255u;
    }

    *last_run = r;
    *alt_val = next_alt_val;
    *last_val = next_last_val;
    *pos = end_pos;
}

static uint32_t rhv2_compress_plane(const uint8_t *plane, uint32_t n, uint8_t *dst) {
    rhv2_init_table();

    rhv2_bitwriter_t bw = {
        .out = dst,
        .len = 0u,
        .cap = n + 16u,
        .acc = 0u,
        .count = 0u
    };

    uint32_t last_run = 1u;
    uint32_t repeat_run = 1u;
    uint8_t alt_val = 0u;
    uint8_t last_val = 255u;
    int mode = 2;
    uint32_t pos = 0u;

    while (pos < n && bw.len <= n) {
        uint8_t v = plane[pos];

        /* Case A: Run of last_val */
        if (v == last_val) {
            uint32_t end_pos = pos + 1u;
            while (end_pos < n && plane[end_pos] == last_val) {
                end_pos++;
            }
            uint32_t r = end_pos - pos;
            if (mode == 2) {
                bw_put(&bw, 0x03u, 3u);
            } else {
                bw_put(&bw, 0x07u, 4u);
            }
            emit_rle(
                &bw, r, end_pos, n, (r == last_run),
                last_val, alt_val,
                &last_run, &repeat_run, &last_val, &alt_val, &pos
            );
            continue;
        }

        /* Case B: Run of alt_val with Large Magnitude Step */
        if (v == alt_val) {
            int diff = (int)v - (int)last_val;
            uint32_t mag = (diff >= 0)
                ? ((uint32_t)diff & 0xFFu)
                : ((uint32_t)(-((int)v + (int)last_val)) & 0xFFu);
            if ((mag & 0xF8u) != 0u) {
                uint32_t end_pos = pos + 1u;
                while (end_pos < n && plane[end_pos] == v) {
                    end_pos++;
                }
                uint32_t r = end_pos - pos;
                if (mode == 2) {
                    bw_put(&bw, 0x01u, 2u);
                } else {
                    bw_put(&bw, 0x0Fu, 5u);
                }
                mode = 2;
                emit_rle(
                    &bw, r, end_pos, n, (r == last_run),
                    v, last_val,
                    &last_run, &repeat_run, &last_val, &alt_val, &pos
                );
                continue;
            }
        }

        /* Case C: Final Pixel of Plane OR Immediate Pair/Run of New Value */
        if (pos + 1u == n || plane[pos + 1u] == v) {
            uint8_t delta = (pos == 0u) ? v : (uint8_t)((v - plane[pos - 1u]) & 0xFFu);
            alt_val = delta;

            uint32_t r;
            uint32_t end_pos;
            if (pos + 1u == n) {
                r = 0u;
                end_pos = n;
            } else {
                end_pos = pos + 2u;
                while (end_pos < n && plane[end_pos] == v) {
                    end_pos++;
                }
                r = end_pos - (pos + 1u);
            }

            if (mode == 4 && r == repeat_run) {
                bw_put(&bw, 0x00u, 1u);
                r = 0u;
                last_run = repeat_run;
            } else {
                if (mode == 2) {
                    bw_put(&bw, 0x07u, 4u);
                } else {
                    bw_put(&bw, 0x01u, 2u);
                }
            }
            bw_put_sym(&bw, delta);
            mode = 4;

            if (r == 0u) {
                alt_val = last_val;
                last_val = v;
                pos = end_pos;
            } else {
                emit_rle(
                    &bw, r, end_pos, n, (r == last_run),
                    v, last_val,
                    &last_run, &repeat_run, &last_val, &alt_val, &pos
                );
            }
            continue;
        }

        /* Case D: Literal Delta Stream Until End-of-Plane or 3 Identical Pixels */
        if (mode == 3) {
            bw_put(&bw, 0x00u, 1u);
        } else if (mode == 2) {
            bw_put(&bw, 0x0Fu, 5u);
        } else {
            bw_put(&bw, 0x1Fu, 6u);
        }

        uint32_t k = pos;
        for (;;) {
            uint8_t u = plane[k];
            if (u != last_val) {
                alt_val = last_val;
            }
            uint8_t delta = (k == 0u) ? u : (uint8_t)((u - plane[k - 1u]) & 0xFFu);
            bw_put_sym(&bw, delta);
            last_val = u;

            uint32_t k_next = k + 1u;
            if (k_next == n) {
                pos = n;
                break;
            }
            if (k_next + 1u < n && plane[k_next] == u && plane[k_next + 1u] == u) {
                bw_put(&bw, 0x7Fu, 7u);
                uint32_t end_pos = k_next + 2u;
                while (end_pos < n && plane[end_pos] == u) {
                    end_pos++;
                }
                uint32_t r = end_pos - k_next;
                mode = 3;
                emit_rle(
                    &bw, r, end_pos, n, (r == last_run),
                    u, alt_val,
                    &last_run, &repeat_run, &last_val, &alt_val, &pos
                );
                break;
            }
            k = k_next;
        }
    }

    bw_flush(&bw);

    if (bw.len > n) {
        dst[0] = 0x7Fu;
        memcpy(dst + 1, plane, n);
        return n + 1u;
    }

    return bw.len;
}

static inline void write_be32(uint8_t *p, uint32_t v) {
    p[0] = (uint8_t)((v >> 24) & 0xFFu);
    p[1] = (uint8_t)((v >> 16) & 0xFFu);
    p[2] = (uint8_t)((v >> 8) & 0xFFu);
    p[3] = (uint8_t)(v & 0xFFu);
}

uint32_t rhv2_compress_band(
    const uint8_t *src_rgbx,
    uint8_t *dst,
    uint32_t aligned_width,
    uint32_t height
) {
    if (!src_rgbx || !dst || aligned_width == 0u || height == 0u) {
        return 0u;
    }

    uint32_t n = aligned_width * height;
    uint8_t *plane_buf = (uint8_t *)malloc(n);
    if (!plane_buf) {
        return 0u;
    }

    uint32_t offset = 16u;
    for (uint32_t p = 0u; p < 4u; ++p) {
        for (uint32_t i = 0u; i < n; ++i) {
            plane_buf[i] = src_rgbx[i * 4u + p];
        }
        uint32_t lp = rhv2_compress_plane(plane_buf, n, dst + offset);
        write_be32(dst + p * 4u, lp);
        offset += lp;
        uint32_t pad = (8u - (lp & 7u)) & 7u;
        for (uint32_t z = 0u; z < pad; ++z) {
            dst[offset++] = 0u;
        }
    }

    free(plane_buf);
    return offset;
}

uint32_t rhv2_compress_rgb_band(
    const uint8_t *src_rgb,
    uint8_t *dst,
    uint32_t print_width,
    uint32_t height
) {
    if (!src_rgb || !dst || print_width == 0u || height == 0u) {
        return 0u;
    }

    uint32_t aligned_width = (print_width + 7u) & ~7u;
    uint32_t n = aligned_width * height;
    uint8_t *planes = (uint8_t *)malloc(n * 3u);
    if (!planes) {
        return 0u;
    }

    uint8_t *plane_r = planes;
    uint8_t *plane_g = planes + n;
    uint8_t *plane_b = planes + n * 2u;

    for (uint32_t y = 0u; y < height; ++y) {
        const uint8_t *src_row = src_rgb + (size_t)y * print_width * 3u;
        uint32_t row_off = y * aligned_width;
        for (uint32_t x = 0u; x < print_width; ++x) {
            plane_r[row_off + x] = src_row[x * 3u + 0u];
            plane_g[row_off + x] = src_row[x * 3u + 1u];
            plane_b[row_off + x] = src_row[x * 3u + 2u];
        }
        for (uint32_t x = print_width; x < aligned_width; ++x) {
            plane_r[row_off + x] = 255u;
            plane_g[row_off + x] = 255u;
            plane_b[row_off + x] = 255u;
        }
    }

    uint32_t offset = 16u;
    for (uint32_t p = 0u; p < 3u; ++p) {
        const uint8_t *plane_ptr = planes + p * n;
        uint32_t lp = rhv2_compress_plane(plane_ptr, n, dst + offset);
        write_be32(dst + p * 4u, lp);
        offset += lp;
        uint32_t pad = (8u - (lp & 7u)) & 7u;
        for (uint32_t z = 0u; z < pad; ++z) {
            dst[offset++] = 0u;
        }
    }

    /* 4th plane (X = 0 for all N bytes) */
    memset(plane_r, 0, n);
    uint32_t lx = rhv2_compress_plane(plane_r, n, dst + offset);
    write_be32(dst + 12u, lx);
    offset += lx;
    uint32_t pad_x = (8u - (lx & 7u)) & 7u;
    for (uint32_t z = 0u; z < pad_x; ++z) {
        dst[offset++] = 0u;
    }

    free(planes);
    return offset;
}
