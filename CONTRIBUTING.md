# Contributing to `epson-p900-escpr-cups`

Thank you for your interest in contributing to `epson-p900-escpr-cups`! Hardware test reports, bug reports, documentation improvements, and code contributions are welcome.

---

## 1. Hardware Test Reports

Because not every paper type, paper source (Rear, Front Fine Art, Front Thick, Roll), and page size has been physically printed on hardware yet (see [`docs/supported-features.md`](docs/supported-features.md)), **hardware test reports** are one of the most valuable contributions:

- Open a **[Hardware Test Report](.github/ISSUE_TEMPLATE/hardware_report.yml)** issue and note:
  - Printer model (`SC-P900` or `SC-P700`) and connection (`socket://...:9100`, `dnssd://`, `usb://`)
  - Paper type (`MediaType` or custom slot), paper source (`InputSlot`), page size (`PageSize`), quality (`PrintQuality`), and color mode (`EpsonColorMode`)
  - Whether the print succeeded as expected or showed any issue (if it failed, attach the `.prn` output captured with `-o job.prn` or the CUPS `error_log` lines).

---

## 2. Building & Testing Locally

### Prerequisites

- Rust `>= 1.85` (Edition 2024)
- LittleCMS 2 (`lcms2-devel` on Fedora/openSUSE, `liblcms2-dev` on Debian/Ubuntu, `lcms2` on Arch)
- CUPS (`cupstestppd` for validating generated PPDs)
- `7z` (`p7zip`) and `curl` (optional, for `./icc/fetch_icc_profiles.sh`)

### Build & Run Checks

Before opening a pull request, make sure all checks enforced by CI (`.github/workflows/ci.yml`) pass cleanly:

```bash
# 1. Check formatting
cargo fmt --all -- --check

# 2. Run Clippy with warnings denied
cargo clippy --all-targets -- -D warnings

# 3. Run unit and integration tests
cargo test

# 4. Validate the reference PPDs with cupstestppd
cupstestppd ppd/Epson-SC-P900-ESCPR.ppd ppd/Epson-SC-P700-ESCPR.ppd
```

---

## 3. Clean-Room & Legal Guidelines

- **No proprietary Epson binaries or ICC profiles in the repository:** Never commit `.icc` profiles, binary driver blobs, or extracted vendor files. Official Epson ICC profiles are downloaded locally by the user via `./icc/fetch_icc_profiles.sh`.
- **No personal data:** Do not commit personal printer serial numbers, MAC addresses, or private paper lists (`custom_media.json` is git-ignored; use `examples/custom_media.example.json` for examples).
- **License:** All contributions are licensed under the **GNU General Public License v3.0 or later (`GPL-3.0-or-later`)**.

---

## 4. Pull Request Workflow

1. Fork the repository and create a topic branch from `main`.
2. Make focused, atomic commits with clear commit messages (e.g. `fix(ppd): ...`, `feat(filter): ...`, `docs: ...`).
3. Verify that `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass.
4. Open a Pull Request describing the motivation, changes, and how they were tested.
