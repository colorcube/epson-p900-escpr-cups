#!/usr/bin/env python3
"""
Wrapper that invokes `rastertop900escpr --generate-feature-table` to generate
`docs/supported-features.md`.

SPDX-License-Identifier: GPL-3.0-or-later
"""

import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / "release" / "rastertop900escpr"

if not BIN.exists():
    subprocess.run(["cargo", "build", "--release", "--offline"], cwd=ROOT, check=True)

cmd = [str(BIN), "--generate-feature-table"] + sys.argv[1:]
sys.exit(subprocess.call(cmd, cwd=ROOT))
