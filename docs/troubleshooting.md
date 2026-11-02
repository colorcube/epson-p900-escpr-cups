# Troubleshooting

## "Paper out in roll paper unit" / the printer asks for roll paper

The printer received a job it could not parse correctly. With current versions
this should not happen any more (see [protocol.md](protocol.md#lessons-learned)).
If it does:

1. Cancel the job on the printer (it may have to be cancelled twice).
2. Write the job to a file instead of printing (`-o job.prn`) and open an issue
   with the file and the options you used.

## Large prints stop half-way and the sheet is ejected

The connection to the printer timed out. The printer slows down the data
stream while it prints and can pause for more than a minute. Use a current
version of the filter; `--send` no longer applies a timeout after connecting.
When printing through CUPS, use the `socket://` backend without a timeout.

## "Could not find Epson ICC profile"

The Epson ICC profiles are not part of this repository. Run
`./icc/fetch_icc_profiles.sh` (see [../icc/README.md](../icc/README.md)) and
reinstall.

## Colors look washed out, too saturated, or double-profiled

- **Standard CUPS printing (`EpsonColorMode=EpsonICC`, default):** `gstoraster`
  rasterizes incoming PDFs into 8-bit `sRGB`, and `rastertop900escpr` converts
  `sRGB -> Epson Paper ICC` once via `liblcms2`. Note that our PPDs intentionally
  use `*%cupsICCProfile` comments rather than active `*cupsICCProfile`
  directives so `cupsd` / `colord` never apply a second ICC transformation in
  Ghostscript upstream of `rastertop900escpr`.
- **Wide-gamut photo printing (`LumaPrint`, darktable, GIMP, Krita):** When your
  photo application converts from **Adobe RGB** or **ProPhoto RGB** into the
  Epson paper ICC profile itself, select `Color Management Mode = No Color
  Adjustment (Host ICC)` (`-o EpsonColorMode=NoColorAdjustment` or
  `-o cm-calibration=true`) so the driver passes your pre-profiled device RGB
  values through unchanged.
- **Standalone CLI (`--image`):** Reads the embedded ICC profile (e.g. Adobe RGB
  or ProPhoto RGB) directly from the image file and converts to the paper ICC
  profile in one step. Images without an embedded profile are treated as sRGB.
- Check that the paper selected in the job (`MediaType`) matches the paper loaded
  in the printer.

## Nothing is printed, but the printer received data

A completely white page is skipped by the printer (`SkipBlankPage`). Check that
the image actually lies inside the printable area.

## The printer does not react at all

- Port 9100 must be reachable: `nc -vz <printer-ip> 9100`.
- Another job may be waiting on the printer's control panel for confirmation.

## `lpadmin: Printer drivers are deprecated and will stop working in a future version of CUPS`

CUPS 2.3+ prints this warning whenever **any** printer queue is registered with
a classic PPD file (`-P` / `-m`), including Gutenprint, HPLIP, TurboPrint, and
vendor drivers. It refers to the long-term plan for a future CUPS 3.0 release to
replace PPD-based filters with standalone IPP Everywhere Printer Applications
(daemons). On all current Linux distributions using CUPS 2.4.x, PPD-based
queues and filters work normally and the message can be safely ignored.

## Inspecting the CUPS log (`error_log` / `journalctl`)

`rastertop900escpr` writes diagnostic messages (resolved media type, ICC profile
path, rendering intent, custom slot parameters, and band count) with `DEBUG:` and
`ERROR:` prefixes to `stderr`, which CUPS captures in its log:

```bash
# Enable CUPS debug logging so DEBUG: lines from rastertop900escpr are recorded
sudo cupsctl --debug-logging

# Follow CUPS and filter messages in real time (systemd journals, e.g. Fedora/Arch)
sudo journalctl -u cups -f

# Or inspect /var/log/cups/error_log directly (Debian/Ubuntu/Fedora)
sudo grep -E "rastertop900escpr|PID" /var/log/cups/error_log | tail -n 50

# Disable verbose logging again when done
sudo cupsctl --no-debug-logging
```

## SELinux blocks the filter, PPD generator, or custom media (Fedora / RHEL)

On distributions with SELinux in enforcing mode, files in `/usr/lib/cups/filter/`,
`/usr/lib/cups/driver/`, `/usr/libexec/epson-p900-escpr-cups/`,
`/etc/epson-p900-escpr-cups/`, and `/usr/local/share/color/icc/epson-p900-custom/`
must carry the expected file contexts (`bin_t`, `etc_t`, `usr_t`). Both
`install.sh` and `epson-p900-media-install` run `restorecon` automatically, but
if a print job fails with `Filter failed` or custom media do not appear:

```bash
# 1. Check for recent SELinux AVC denials
sudo ausearch -m avc -ts recent

# 2. Restore default SELinux contexts on all driver and media paths
sudo restorecon -RFv \
    /usr/lib/cups/filter/rastertop900escpr \
    /usr/lib/cups/driver/epson-p900-escpr \
    /usr/libexec/epson-p900-escpr-cups \
    /etc/epson-p900-escpr-cups \
    /usr/share/color/icc/epson-scp900 \
    /usr/local/share/color/icc/epson-p900-custom
```

## "Install for this computer" (`pkexec`) fails or shows no password prompt

When clicking **“Install for this computer”** in `epmi-gui` (or running
`epmi install-system`), `epmi` invokes
`pkexec /usr/libexec/epson-p900-escpr-cups/epson-p900-media-install`.

- **No password dialog appears / `No authentication agent found`:** Your desktop
  session does not have a Polkit authentication agent running (common on minimal
  window managers like Sway, Hyprland, or i3). Either start an agent (e.g.
  `/usr/libexec/kf6/polkit-kde-authentication-agent-1` or
  `polkit-gnome-authentication-agent-1`), or run `epmi install-system` from an
  interactive terminal where `pkexec` can fall back to its text-mode prompt.
- **`epson-p900-media-install` not found:** Make sure `epson-p900-escpr-cups`
  (`>= 0.3.0`) was installed with `sudo ./install.sh`.

## Printer is not discovered or NVRAM slots cannot be read via SNMP (`epmi`)

`ep-media-installer` (`epmi`) queries and writes the 30 custom media slots over
**SNMPv1 (`UDP` port `161`, community `public` / `private`)**:

1. **Test UDP 161 reachability:** Run `epmi list-slots --host <PRINTER_IP>`.
2. **Check printer Web Config:** Open `http://<PRINTER_IP>` in a browser and
   verify that **SNMPv1/v2c** (and Community String `public`) is enabled under
   *Network → Network Security / Services → SNMP*.
3. **Firewall / VPN / Subnet:** Automatic broadcast/subnet discovery scans the
   local `/24` interface and CUPS device URIs. If your computer is connected to a
   VPN or a routed VLAN, pass `--host <PRINTER_IP>` explicitly (or enter the IP
   address in the `epmi-gui` top bar and click **`Connect`**).

