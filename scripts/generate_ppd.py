#!/usr/bin/env python3
"""
Wrapper that invokes `rastertop900escpr --generate-ppd` to generate
`ppd/Epson-SC-P900-ESCPR.ppd` and `ppd/Epson-SC-P700-ESCPR.ppd`.

SPDX-License-Identifier: GPL-3.0-or-later
"""

import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / "release" / "rastertop900escpr"

if not BIN.exists():
    subprocess.run(["cargo", "build", "--release", "--offline"], cwd=ROOT, check=True)

sys.exit(subprocess.call([str(BIN), "--generate-ppd", "--ppd-dir", str(ROOT / "ppd")]))
