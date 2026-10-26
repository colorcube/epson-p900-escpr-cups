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

## Colors look washed out or too saturated

- Check that the image has the right embedded ICC profile. Images without a
  profile are treated as sRGB; an Adobe RGB image without an embedded profile
  will print too saturated.
- Check that the paper selected in the job (`MediaType`) matches the paper in
  the printer **and** the paper type set on the printer's control panel.
- `EpsonColorMode=NoColorAdjustment` sends RGB unchanged – only use it if your
  application already converted the image with a printer profile.

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

