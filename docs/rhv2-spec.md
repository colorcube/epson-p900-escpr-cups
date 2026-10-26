# ESC/P-R Level 5 (`bsn2`) Planar Compression Format (`RHV2`) Specification

## 1. Overview and Band Container Layout

In ESC/P-R Level 5 (`ESC d <len> bsn2` with `CompMode = 1`), each horizontal image band of width $W$ pixels and height $H$ rows is transmitted as four planar bitstreams representing Red ($R$), Green ($G$), Blue ($B$), and an unused fourth channel ($X = 0$).

### 1.1 Horizontal Alignment and Padding
- The logical band width $W$ is padded up to the next multiple of 8 pixels:
  $$W_{\text{aligned}} = (W + 7) \mathbin{\&} \sim 7$$
- For each row $y \in [0, H - 1]$, pixels at $x \in [W, W_{\text{aligned}} - 1]$ are padded with white:
  $$(R, G, B, X) = (255, 255, 255, 0)$$
- Each plane therefore contains $N = W_{\text{aligned}} \times H$ bytes scanned in row-major order.

### 1.2 Container Structure
The compressed band payload consists of:
1. **16-Byte Header:** Four 32-bit unsigned big-endian integers $(L_R, L_G, L_B, L_X)$ giving the unpadded compressed byte length of each of the four planes.
2. **Four Aligned Plane Payloads:** Each plane's compressed byte stream of length $L_p$ is immediately followed by $(8 - (L_p \bmod 8)) \bmod 8$ zero padding bytes (`0x00`) so that every plane starts and ends on an 8-byte boundary relative to the start of the band payload.

---

## 2. Bitstream Packing Order

Each plane is encoded into a sequence of variable-length bitfields packed **least-significant-bit (LSB) first**:
- Maintain an unsigned bit accumulator `acc` (initially `0`) and bit count `count` (initially `0`).
- Appending an $m$-bit value $v \in [0, 2^m - 1]$ performs:
  $$\text{acc} \leftarrow \text{acc} \mathbin{|} (v \ll \text{count}), \quad \text{count} \leftarrow \text{count} + m$$
- Whenever $\text{count} \ge 8$, emit the lowest byte $(\text{acc} \mathbin{\&} \text{0xFF})$ to the output stream, shift $\text{acc} \leftarrow \text{acc} \gg 8$, and decrement $\text{count} \leftarrow \text{count} - 8$.
- After the final pixel of the plane is encoded, if $\text{count} > 0$, emit one final byte $(\text{acc} \mathbin{\&} \text{0xFF})$ (implicitly zero-padded in the upper $8 - \text{count}$ bits).

### 2.1 Uncompressed Plane Escape
If the resulting compressed byte length $L_p$ of a plane strictly exceeds $N$ ($L_p > N$), the compressed bitstream for that plane is discarded and replaced by $1 + N$ bytes: a single escape byte `0x7F` followed by the $N$ raw plane bytes.

---

## 3. Variable-Length Symbol Code Table

A single 256-entry prefix code table $(C[s], B[s])$ maps an 8-bit unsigned symbol $s \in [0, 255]$ to an LSB-first code $C[s]$ of length $B[s]$ bits. This table is used both for 8-bit pixel deltas and for run-length chunks:

| Symbol Value(s) $s$ | Bit Length $B[s]$ | Code $C[s]$ Formula (LSB-first integer) |
|---|---|---|
| $s = 0$ | $3$ | `0b101` (`0x05`) |
| $s = 1$ | $3$ | `0b100` (`0x04`) |
| $s = 255$ | $3$ | `0b000` (`0x00`) |
| $s = 128$ | $7$ | `0b0111111` (`0x3F`) |
| $k \in [2, 3]$ | $4$ | $C[k] = (k \ll 2) \mathbin{|} \text{0x02}$, $C[256 - k] = ((3 - k) \ll 2) \mathbin{|} \text{0x02}$ |
| $k \in [4, 7]$ | $6$ | $C[k] = (k \ll 3) \mathbin{|} \text{0x01}$, $C[256 - k] = ((7 - k) \ll 3) \mathbin{|} \text{0x01}$ |
| $k \in [8, 15]$ | $7$ | $C[k] = (k \ll 3) \mathbin{|} \text{0x03}$, $C[256 - k] = ((15 - k) \ll 3) \mathbin{|} \text{0x03}$ |
| $k \in [16, 31]$ | $9$ | $C[k] = (k \ll 4) \mathbin{|} \text{0x07}$, $C[256 - k] = ((31 - k) \ll 4) \mathbin{|} \text{0x07}$ |
| $k \in [32, 63]$ | $11$ | $C[k] = (k \ll 5) \mathbin{|} \text{0x0F}$, $C[256 - k] = ((63 - k) \ll 5) \mathbin{|} \text{0x0F}$ |
| $k \in [64, 127]$ | $13$ | $C[k] = (k \ll 6) \mathbin{|} \text{0x1F}$, $C[256 - k] = ((127 - k) \ll 6) \mathbin{|} \text{0x1F}$ |

---

## 4. Run-Length Encoding Procedure

When a horizontal run of length $R \ge 1$ ending at exclusive index $\text{end\_pos} \le N$ is emitted via the run-length encoder with parameters `(R, end_pos, is_same_run, next_last_val, next_alt_val)`:

1. If `is_same_run` is true (i.e., $R$ equals the current `last_run`), update `repeat_run` $\leftarrow R$.
2. Determine the remaining count to encode:
   - If $\text{end\_pos} == N$ (the run reaches the end of the plane), set $\text{rem} \leftarrow 0$.
   - Otherwise ($\text{end\_pos} < N$), set $\text{rem} \leftarrow R$.
3. Loop:
   - Let $\text{chunk} = \min(\text{rem}, 255)$.
   - Map $\text{chunk}$ to symbol $s \in [0, 255]$:
     - If $\text{chunk} == 0$: $s = 128$ (`0x80`).
     - Else if $\text{chunk}$ is odd ($\text{chunk} \bmod 2 == 1$): $s = \lfloor \text{chunk} / 2 \rfloor$.
     - Else ($\text{chunk}$ is even and non-zero): $s = (256 - (\text{chunk} / 2)) \bmod 256$.
   - Emit code $(C[s], B[s])$ to the bitstream.
   - If $\text{rem} \le 254$, exit the loop; otherwise set $\text{rem} \leftarrow \text{rem} - 255$ and repeat.
4. Update the encoder registers:
   - `last_run` $\leftarrow R$
   - `alt_val` $\leftarrow$ `next_alt_val`
   - `last_val` $\leftarrow$ `next_last_val`
   - `pos` $\leftarrow \text{end\_pos}$

---

## 5. Plane State Machine

At the start of each plane $P[0 \dots N-1]$, initialize:
- `last_run` $= 1$
- `repeat_run` $= 1$
- `alt_val` $= 0$ (8-bit unsigned)
- `last_val` $= 255$ (`0xFF`, 8-bit unsigned)
- `mode` $= 2$ (integer state in $\{2, 3, 4\}$)
- `pos` $= 0$

While $\text{pos} < N$, let $v = P[\text{pos}]$ and evaluate the following four cases in strict priority order:

### Case A: Run of `last_val` ($v == \text{last\_val}$)
1. Scan forward from $\text{pos} + 1$ to find the first index $\text{end\_pos} \le N$ where $P[\text{end\_pos}] \ne \text{last\_val}$, and let $R = \text{end\_pos} - \text{pos}$.
2. Emit mode prefix:
   - If $\text{mode} == 2$: emit $3$-bit code `0x3` (`0b011`).
   - Else: emit $4$-bit code `0x7` (`0b0111`).
   - (`mode` itself is unchanged.)
3. Invoke the Run-Length Encoding Procedure (Section 4) with `(R, end_pos, R == last_run, last_val, alt_val)` and continue the main loop.

### Case B: Run of `alt_val` with Large Magnitude Step
This case applies when $v == \text{alt\_val}$ **and** the 8-bit magnitude test holds:
- Let $\text{diff} = \text{ int}(v) - \text{ int}(\text{last\_val})$.
- Let $\text{mag} = (\text{diff} \ge 0) \mathbin{?} (\text{diff} \mathbin{\&} \text{0xFF}) : ((-(\text{int}(v) + \text{int}(\text{last\_val}))) \mathbin{\&} \text{0xFF})$.
- Condition: $(\text{mag} \mathbin{\&} \text{0xF8}) \ne 0$.

When both $v == \text{alt\_val}$ and $(\text{mag} \mathbin{\&} \text{0xF8}) \ne 0$ hold:
1. Scan forward from $\text{pos} + 1$ to find $\text{end\_pos} \le N$ while $P[\text{end\_pos}] == v$, and let $R = \text{end\_pos} - \text{pos}$.
2. Emit mode prefix:
   - If $\text{mode} == 2$: emit $2$-bit code `0x1` (`0b01`).
   - Else: emit $5$-bit code `0x0F` (`0b01111`).
3. Set $\text{mode} \leftarrow 2$.
4. Invoke the Run-Length Encoding Procedure (Section 4) with `(R, end_pos, R == last_run, v, last_val)` and continue the main loop.

### Case C: Final Pixel of Plane OR Immediate Pair/Run of New Value
This case applies when $\text{pos} + 1 == N$ **or** $P[\text{pos} + 1] == v$:
1. Compute the 8-bit delta $\Delta = (\text{pos} == 0) \mathbin{?} v : ((v - P[\text{pos} - 1]) \mathbin{\&} \text{0xFF})$.
2. Temporarily set $\text{alt\_val} \leftarrow \Delta$.
3. Determine the trailing run length $R$ after the first pixel at $\text{pos}$:
   - If $\text{pos} + 1 == N$: $R = 0$ and $\text{end\_pos} = N$.
   - Else: scan forward from $\text{pos} + 2$ to find $\text{end\_pos} \le N$ while $P[\text{end\_pos}] == v$, and set $R = \text{end\_pos} - (\text{pos} + 1)$ (so $R \ge 1$).
4. Emit prefix and delta:
   - If $\text{mode} == 4$ **and** $R == \text{repeat\_run}$:
     - Emit $1$-bit code `0x0` (`0b0`).
     - Set $R \leftarrow 0$ and $\text{last\_run} \leftarrow \text{repeat\_run}$.
   - Else:
     - If $\text{mode} == 2$: emit $4$-bit code `0x7` (`0b0111`).
     - Else: emit $2$-bit code `0x1` (`0b01`).
   - Emit symbol code $(C[\Delta], B[\Delta])$.
5. Set $\text{mode} \leftarrow 4$.
6. If $R == 0$:
   - Set $\text{alt\_val} \leftarrow \text{last\_val}$, $\text{last\_val} \leftarrow v$, and $\text{pos} \leftarrow \text{end\_pos}$.
7. Else ($R > 0$):
   - Invoke the Run-Length Encoding Procedure (Section 4) with `(R, end_pos, R == last_run, v, last_val)`.
8. Continue the main loop.

### Case D: Literal Delta Stream Until End-of-Plane or 3 Identical Pixels
If none of Cases A, B, or C matched:
1. Emit literal-entry prefix based on `mode`:
   - If $\text{mode} == 3$: emit $1$-bit code `0x0` (`0b0`).
   - Else if $\text{mode} == 2$: emit $5$-bit code `0x0F` (`0b01111`).
   - Else ($\text{mode} == 4$): emit $6$-bit code `0x1F` (`0b011111`).
2. Set cursor $k \leftarrow \text{pos}$ and repeat:
   - Let $u = P[k]$.
   - If $u \ne \text{last\_val}$, set $\text{alt\_val} \leftarrow \text{last\_val}$.
   - Compute 8-bit delta $\Delta = (k == 0) \mathbin{?} u : ((u - P[k - 1]) \mathbin{\&} \text{0xFF})$.
   - Emit symbol code $(C[\Delta], B[\Delta])$.
   - Set $\text{last\_val} \leftarrow u$.
   - Let $k_{\text{next}} = k + 1$.
   - If $k_{\text{next}} == N$: set $\text{pos} \leftarrow N$ and break out of the literal loop.
   - Check whether a run of at least 3 identical bytes equal to $u$ starts at $k_{\text{next}}$:
     - If $k_{\text{next}} + 1 < N$ **and** $P[k_{\text{next}}] == u$ **and** $P[k_{\text{next}} + 1] == u$:
       - Emit the 7-bit literal-termination marker `0x7F` (`0b1111111`).
       - Scan forward from $k_{\text{next}} + 2$ to find $\text{end\_pos} \le N$ while $P[\text{end\_pos}] == u$, and let $R = \text{end\_pos} - k_{\text{next}}$.
       - Set $\text{mode} \leftarrow 3$.
       - Invoke the Run-Length Encoding Procedure (Section 4) with `(R, end_pos, R == last_run, u, alt_val)` and break out of the literal loop.
     - Otherwise, set $k \leftarrow k_{\text{next}}$ and continue the literal loop.

---

## 6. Reference Test Vectors

### Vector 1: 8-Byte Constant White Plane
- **Input plane ($N = 8$):** `[255, 255, 255, 255, 255, 255, 255, 255]`
- **Expected compressed plane hex:** `fb01`
- **All-white RGB band payload (any width/height):**
  ```
  00000002000000020000000200000002
  fb01000000000000fb01000000000000
  fb01000000000000d71f000000000000
  ```

### Vector 2: 8-Byte Mixed Literal & Run Plane
- **Input plane ($N = 8$):** `[0, 1, 2, 5, 5, 5, 200, 200]`
- **Expected compressed plane hex:** `afa4ffd1137e`

### Vector 3: Synthetic $17 \times 2$ RGB Band (Tests Alignment Padding to $24 \times 2$)
- **Input ($17 \times 2 = 34$ RGB pixels):**
  For $i \in [0, 33]$:
  - $R_i = (7i) \bmod 256$
  - $G_i = 128$ if $4 \le i < 12$ else $(3i) \bmod 256$
  - $B_i = 255$ if $(i \bmod 5 \ne 0)$ else $240$
- **Expected band output length:** `112` bytes
- **Expected band SHA-256:** `5800369faa25249774a666865ee2896f3b620988993bb00d3db41d683bbb6b06`
- **Expected band hex:**
  ```
  00000021000000190000000e00000002
  af799ee7799ee7799ee7799ee79fe32f3e7c9ee7799ee7799ee7799ee7f9e1ff0f00000000000000
  afeefeddffee1bb9bb3fe72f1eedeeeeeeeeeeeeeefe6dfe3f00000000000000
  efdefecfd7ac66b5b46635abfd000000
  d71f000000000000
  ```

### Vector 4: Full State-Transition Plane ($N = 328$)
- **Input plane construction (Python expression):**
  ```python
  bytes(
      [255] * 5
      + [10, 10, 10]
      + [255, 255]
      + [10, 10, 10]
      + [20, 20, 20]
      + [30, 30, 30]
      + [40, 42, 45, 50, 50, 50]
      + [60, 63, 67, 70]
      + [80] * 300
      + [90]
  )
  ```
- **Expected compressed plane hex (`20` bytes):** `d3dbe221de14a6df543dfdc3d4439ffebfffa5a6`

### Vector 5: High-Entropy Uncompressed Fallback ($N = 64$)
- **Input plane:** `bytes([(i * 73 + 19) & 0xFF for i in range(64)])`
- **Expected compressed plane:** `65` bytes starting with `0x7F` followed by the 64 raw input bytes.
