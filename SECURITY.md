# Security Policy

## Supported versions

Security fixes are made for the latest release only.

## What is security-relevant here

- `epson-p900-media-install` runs as **root** (started via `pkexec`, polkit action
  `de.colorcube.epson-p900-escpr.install-media`). It reads a staging directory prepared by an
  unprivileged user and writes `/etc/epson-p900-escpr-cups/`, `/usr/local/share/color/icc/epson-p900-custom/`
  and the PPDs of CUPS queues. Anything that lets a user read, write or execute something they
  could not before is a vulnerability.
- The CUPS filter `rastertop900escpr` (runs as user `lp`) parses raster data and job options of
  print jobs submitted by any user.

## Reporting a vulnerability

Please **do not open a public issue**. Use GitHub's private vulnerability reporting instead:
[Report a vulnerability](https://github.com/colorcube/epson-p900-escpr-cups/security/advisories/new).

Include the affected version, steps to reproduce and, if possible, a proof of concept.
This is a spare-time project: reports are answered as soon as possible, usually within a few weeks.
