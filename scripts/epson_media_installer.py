#!/usr/bin/env python3
"""
epson_media_installer.py - Linux CLI replacement for Epson Media Installer (v2.3.0)
                           supporting the Epson SureColor SC-P900 and SC-P700.

Subcommands:
  list-slots     Query custom media slots 1..30 from the printer via SNMPv1
  sync           Synchronize active printer NVRAM slots into custom_media.json & PPDs
  inspect-emx    Inspect an Epson .emx media package (XML + CPB + ICC)
  import-emx     Import an .emx package into custom_media.json (and optionally write to printer slot)
  write-slot     Write custom paper settings directly into printer NVRAM slot 1..30
  clear-slot     Reset/clear printer NVRAM slot 1..30
  cloud-list     Query Epson's LFP Cloud Content Server for downloadable .emx media
  cloud-download Download an .emx package from Epson's server (and optionally import)

SPDX-License-Identifier: GPL-3.0-or-later
"""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys
from typing import List, Optional

_HERE = pathlib.Path(__file__).resolve().parent
_ROOT = _HERE.parent
sys.path.insert(0, str(_ROOT / "src"))

from escpr_protocol import MEDIA_BY_KEY, get_media_spec  # noqa: E402
from media_installer import (  # noqa: E402
    PrinterMediaSlot,
    clear_bank,
    cloud_download_emx,
    cloud_list_media,
    import_emx_package,
    parse_emx_archive,
    read_all_slots,
    read_bank,
    slot_to_custom_media_entry,
    sync_slots_to_custom_media_json,
    write_bank,
)

DEFAULT_PRINTER_HOST = "10.10.10.124"

PLATEN_GAP_LABELS = {
    9: "0.9 mm (Auto/Narrow)",
    12: "1.2 mm (Standard Narrow)",
    15: "1.5/1.6 mm (Standard)",
    21: "2.1 mm (Wide)",
    24: "2.4 mm (Wider)",
}


def _regenerate_ppds() -> None:
    gen_script = _HERE / "generate_ppd.py"
    subprocess.run([sys.executable, str(gen_script)], check=True)


def _print_slots_table(slots: List[PrinterMediaSlot], show_all: bool = False) -> None:
    rows = [s for s in slots if show_all or s.registered]
    if not rows:
        print("No registered custom media slots found on printer.")
        return
    hdr = (
        f"{'Slot':>4}  {'UI ID':>5}  {'32-bit MA_ID':>12}  "
        f"{'BaseSN':>6}  {'Gap':<22}  {'Thick':>7}  {'Dry':>3}  "
        f"{'CutOff':>6}  {'RollOff':>7}  {'Media Name (Printer LCD)'}"
    )
    print(hdr)
    print("-" * len(hdr))
    for s in rows:
        base_spec = MEDIA_BY_KEY.get(f"sn_{s.base_sn_id}")
        base_str = f"{s.base_sn_id}" + (f" ({base_spec.key})" if base_spec else "")
        gap_str = PLATEN_GAP_LABELS.get(s.platen_gap, f"{s.platen_gap} ({s.platen_gap / 10.0:.1f} mm)")
        name_str = s.media_name if s.registered else "(Unregistered)"
        ui_id = 1900 + s.slot
        print(
            f"{s.slot:>4}  {ui_id:>5}  {s.user_media_id:>12}  "
            f"{s.base_sn_id:>6}  {gap_str:<22}  {s.thickness_mm:>5.2f}mm  "
            f"{s.drying_time:>3}  {s.cut_sheet_feed_offset:>+6d}  {s.roll_paper_feed_offset:>+7d}  "
            f"{name_str} [{base_str}]"
        )


def cmd_list_slots(args: argparse.Namespace) -> int:
    slots = read_all_slots(
        host=args.host,
        only_registered=not args.all,
        timeout=args.timeout,
    )
    if args.json:
        print(json.dumps([slot_to_custom_media_entry(s) for s in slots], indent=2, ensure_ascii=False))
    else:
        _print_slots_table(slots, show_all=args.all)
    return 0


def cmd_sync(args: argparse.Namespace) -> int:
    json_path = pathlib.Path(args.custom_media_json)
    target_path, entries = sync_slots_to_custom_media_json(
        host=args.host,
        json_path=json_path,
    )
    print(f"Synchronized {len(entries)} custom media entries into {target_path}:")
    for item in entries:
        slot = item.get("slot", 0)
        uid = item.get("user_media_id", 0)
        print(
            f"  Slot {slot:2d} | key={str(item.get('key')):<40} | "
            f"MA_ID={uid:<10} | base={item.get('base_media')} (sn={item.get('base_sn_id')}) | "
            f"duty={int(item.get('print_duty', 0)):+d}% | icc={item.get('icc_profile')}"
        )
    if args.regenerate_ppd:
        _regenerate_ppds()
    return 0


def cmd_inspect_emx(args: argparse.Namespace) -> int:
    pkg = parse_emx_archive(args.emx_file)
    base_spec = MEDIA_BY_KEY.get(f"sn_{pkg.base_sn_id}")
    base_name = f"{pkg.base_sn_id} ({base_spec.key})" if base_spec else str(pkg.base_sn_id)
    info = {
        "source_path": pkg.source_path,
        "model_name": pkg.model_name,
        "series_name": pkg.series_name,
        "user_media_id": pkg.user_media_id,
        "media_name": pkg.media_name,
        "media_full_name": pkg.media_full_name,
        "vendor": pkg.vendor,
        "base_sn_id": base_name,
        "mac_ppd_id": pkg.mac_ppd_id,
        "win_drv_id": pkg.win_drv_id,
        "media_category": pkg.media_category,
        "platen_gap": pkg.platen_gap,
        "thickness_01mm": pkg.thickness_01mm,
        "thickness_mm": pkg.thickness_mm,
        "drying_time": pkg.drying_time,
        "paper_width_check": pkg.paper_width_check,
        "cut_sheet_feed_offset": pkg.cut_sheet_feed_offset,
        "roll_paper_feed_offset": pkg.roll_paper_feed_offset,
        "ink_density": pkg.ink_density,
        "print_duty": pkg.print_duty,
        "icc_profile_name": pkg.icc_profile_name,
        "icc_size_bytes": len(pkg.icc_profile_bytes),
    }
    print(json.dumps(info, indent=2, ensure_ascii=False))
    return 0


def cmd_import_emx(args: argparse.Namespace) -> int:
    json_path = pathlib.Path(args.custom_media_json)
    entry = import_emx_package(
        emx_path=args.emx_file,
        slot=args.slot,
        json_path=json_path,
        printer_host=args.host if args.write_printer else None,
    )
    print(f"Imported '{entry['name']}' (MA_ID={entry['user_media_id']}) into {json_path}")
    print(json.dumps(entry, indent=2, ensure_ascii=False))
    if args.regenerate_ppd:
        _regenerate_ppds()
    return 0


def cmd_write_slot(args: argparse.Namespace) -> int:
    base_spec = get_media_spec(args.base_media)
    media_id = args.media_id if args.media_id > 0 else (900000000 + args.slot)
    thickness_01mm = int(round(args.thickness_mm * 100.0))
    slot_obj = PrinterMediaSlot(
        slot=args.slot,
        registered=True,
        media_name=args.name,
        user_media_id=media_id,
        base_sn_id=base_spec.sn_id,
        media_category=args.category,
        platen_gap=args.platen_gap,
        thickness_01mm=thickness_01mm,
        drying_time=args.dry_time,
        paper_width_check=args.width_check,
        cut_sheet_feed_offset=args.cut_offset,
        roll_paper_feed_offset=args.roll_offset,
    )
    write_bank(args.host, slot_obj, timeout=args.timeout)
    verified = read_bank(args.host, args.slot, timeout=args.timeout)
    print(
        f"Wrote and verified Slot {verified.slot}: '{verified.media_name}' "
        f"(MA_ID={verified.user_media_id}, BaseSN={verified.base_sn_id}, "
        f"Gap={verified.platen_gap}, Thickness={verified.thickness_mm:.2f}mm)"
    )
    if args.sync:
        cmd_sync(
            argparse.Namespace(
                host=args.host,
                custom_media_json=args.custom_media_json,
                regenerate_ppd=args.regenerate_ppd,
            )
        )
    return 0


def cmd_clear_slot(args: argparse.Namespace) -> int:
    clear_bank(args.host, args.slot, timeout=args.timeout)
    verified = read_bank(args.host, args.slot, timeout=args.timeout)
    print(f"Cleared Slot {args.slot} (registered={verified.registered})")
    if args.sync:
        cmd_sync(
            argparse.Namespace(
                host=args.host,
                custom_media_json=args.custom_media_json,
                regenerate_ppd=args.regenerate_ppd,
            )
        )
    return 0


def cmd_cloud_list(args: argparse.Namespace) -> int:
    records = cloud_list_media(
        printer=args.printer,
        country=args.country,
        region=args.region,
        locale=args.locale,
    )
    if args.json:
        print(json.dumps(records, indent=2, ensure_ascii=False))
        return 0
    hdr = f"{'EMX ID':<12}  {'Mfr':<12}  {'Kind':<10}  {'Shape':<8}  {'Paper Name'}"
    print(hdr)
    print("-" * len(hdr))
    for r in records:
        emx_id = str(r.get("emx_id") or "-")
        mfr = str(r.get("media_manufacturer_name") or "")[:12]
        kind = "Custom/EMX" if r.get("emx_id") else "Built-in"
        shape = str(r.get("media_shape_id") or "")
        name = str(r.get("media_type_name") or "")
        if args.emx_only and not r.get("emx_id"):
            continue
        print(f"{emx_id:<12}  {mfr:<12}  {kind:<10}  {shape:<8}  {name}")
    return 0


def cmd_cloud_download(args: argparse.Namespace) -> int:
    records = cloud_list_media(
        printer=args.printer,
        country=args.country,
        region=args.region,
        locale=args.locale,
    )
    match = None
    for r in records:
        if str(r.get("emx_id") or "") == str(args.emx_id):
            match = r
            break
    if match is None:
        raise SystemExit(f"Could not find EMX ID '{args.emx_id}' for printer={args.printer} region={args.region}")

    out_path = cloud_download_emx(match, out_dir=args.out_dir)
    print(f"Downloaded {out_path} ({out_path.stat().st_size} bytes)")
    if args.import_package:
        return cmd_import_emx(
            argparse.Namespace(
                emx_file=str(out_path),
                slot=args.slot,
                custom_media_json=args.custom_media_json,
                host=args.host,
                write_printer=args.write_printer,
                regenerate_ppd=args.regenerate_ppd,
            )
        )
    return 0


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        description="Linux Custom Media & .emx Manager for Epson SureColor SC-P900 / SC-P700"
    )
    sub = p.add_subparsers(dest="subcommand", required=True)

    # list-slots
    p_list = sub.add_parser("list-slots", help="List custom media slots 1..30 from printer NVRAM")
    p_list.add_argument("--host", default=DEFAULT_PRINTER_HOST, help="Printer IP/hostname")
    p_list.add_argument("--timeout", type=float, default=3.0, help="SNMP timeout in seconds")
    p_list.add_argument("--all", action="store_true", help="Show all 30 slots including empty ones")
    p_list.add_argument("--json", action="store_true", help="Output JSON")
    p_list.set_defaults(func=cmd_list_slots)

    # sync
    p_sync = sub.add_parser(
        "sync",
        help="Read active NVRAM slots 1..30 from printer and update custom_media.json",
    )
    p_sync.add_argument("--host", default=DEFAULT_PRINTER_HOST, help="Printer IP/hostname")
    p_sync.add_argument(
        "--custom-media-json",
        default=str(_ROOT / "custom_media.json"),
        help="Path to custom_media.json",
    )
    p_sync.add_argument(
        "--regenerate-ppd",
        action="store_true",
        help="Regenerate CUPS PPD files after syncing",
    )
    p_sync.set_defaults(func=cmd_sync)

    # inspect-emx
    p_insp = sub.add_parser("inspect-emx", help="Inspect an Epson .emx archive")
    p_insp.add_argument("emx_file", help="Path to .emx file")
    p_insp.set_defaults(func=cmd_inspect_emx)

    # import-emx
    p_imp = sub.add_parser(
        "import-emx",
        help="Import an Epson .emx package into custom_media.json (and optionally write to printer)",
    )
    p_imp.add_argument("emx_file", help="Path to .emx file")
    p_imp.add_argument("--slot", type=int, default=0, help="Target NVRAM slot 1..30 (0 = none)")
    p_imp.add_argument("--host", default=DEFAULT_PRINTER_HOST, help="Printer IP/hostname")
    p_imp.add_argument(
        "--write-printer",
        action="store_true",
        help="Also write the .cpb mechanical parameters into printer NVRAM slot (--slot required)",
    )
    p_imp.add_argument(
        "--custom-media-json",
        default=str(_ROOT / "custom_media.json"),
        help="Path to custom_media.json",
    )
    p_imp.add_argument(
        "--regenerate-ppd",
        action="store_true",
        help="Regenerate CUPS PPD files after importing",
    )
    p_imp.set_defaults(func=cmd_import_emx)

    # write-slot
    p_wr = sub.add_parser("write-slot", help="Write custom media parameters to printer NVRAM slot 1..30")
    p_wr.add_argument("--host", default=DEFAULT_PRINTER_HOST, help="Printer IP/hostname")
    p_wr.add_argument("--timeout", type=float, default=3.0)
    p_wr.add_argument("--slot", type=int, required=True, help="Slot number 1..30")
    p_wr.add_argument("--name", required=True, help="Media name displayed on printer LCD (max 60 chars)")
    p_wr.add_argument(
        "--base-media",
        default="VelvetFineArt",
        help="Base Epson media key (e.g. Baryta, VelvetFineArt, PremiumLuster) or sn_id",
    )
    p_wr.add_argument(
        "--media-id",
        type=int,
        default=0,
        help="32-bit MA_ID (Tag 0x24; 0 = auto 900000000 + slot)",
    )
    p_wr.add_argument(
        "--platen-gap",
        type=int,
        default=21,
        help="Platen gap value in 0.1 mm (e.g. 9=0.9, 12=1.2, 15=1.5, 21=2.1)",
    )
    p_wr.add_argument(
        "--thickness-mm",
        type=float,
        default=0.29,
        help="Paper thickness in mm (0.01..1.50, e.g. 0.29)",
    )
    p_wr.add_argument("--dry-time", type=int, default=0, help="Drying time per pass (0..100)")
    p_wr.add_argument("--cut-offset", type=int, default=0, help="Cut sheet feed offset (-300..+300)")
    p_wr.add_argument("--roll-offset", type=int, default=0, help="Roll paper feed offset (-300..+300)")
    p_wr.add_argument("--category", type=int, default=4, help="Media category (1=Photo, 2=Proofing, 3=Matte, 4=FineArt, 5=Canvas, 6=Plain)")
    p_wr.add_argument("--width-check", type=int, default=1, help="MeasurementPaperWidth (1=On, 0=Off)")
    p_wr.add_argument("--sync", action="store_true", help="Sync custom_media.json after writing")
    p_wr.add_argument(
        "--custom-media-json",
        default=str(_ROOT / "custom_media.json"),
        help="Path to custom_media.json",
    )
    p_wr.add_argument("--regenerate-ppd", action="store_true", help="Regenerate PPDs after sync")
    p_wr.set_defaults(func=cmd_write_slot)

    # clear-slot
    p_clr = sub.add_parser("clear-slot", help="Clear/reset printer NVRAM slot 1..30")
    p_clr.add_argument("--host", default=DEFAULT_PRINTER_HOST, help="Printer IP/hostname")
    p_clr.add_argument("--timeout", type=float, default=3.0)
    p_clr.add_argument("--slot", type=int, required=True, help="Slot number 1..30")
    p_clr.add_argument("--sync", action="store_true", help="Sync custom_media.json after clearing")
    p_clr.add_argument(
        "--custom-media-json",
        default=str(_ROOT / "custom_media.json"),
        help="Path to custom_media.json",
    )
    p_clr.add_argument("--regenerate-ppd", action="store_true", help="Regenerate PPDs after sync")
    p_clr.set_defaults(func=cmd_clear_slot)

    # cloud-list
    p_cl = sub.add_parser("cloud-list", help="List media on Epson Cloud Content Server")
    p_cl.add_argument("--printer", default="scp9xx", help="EmxSeries: scp9xx or scp7xx")
    p_cl.add_argument("--country", default="DE", help="ISO country code")
    p_cl.add_argument("--region", type=int, default=8, help="Region bitmask (4=Europe All, 8=Western Europe)")
    p_cl.add_argument("--locale", default="de", help="Locale code")
    p_cl.add_argument("--emx-only", action="store_true", help="Show only downloadable .emx packages")
    p_cl.add_argument("--json", action="store_true", help="Output raw JSON")
    p_cl.set_defaults(func=cmd_cloud_list)

    # cloud-download
    p_cd = sub.add_parser("cloud-download", help="Download an .emx package from Epson Cloud Content Server")
    p_cd.add_argument("emx_id", help="Epson EMX ID (e.g. 230009089)")
    p_cd.add_argument(
        "--out-dir",
        default=str(_ROOT / "emx"),
        help="Directory to save downloaded .emx file",
    )
    p_cd.add_argument("--printer", default="scp9xx")
    p_cd.add_argument("--country", default="DE")
    p_cd.add_argument("--region", type=int, default=8)
    p_cd.add_argument("--locale", default="de")
    p_cd.add_argument("--import", dest="import_package", action="store_true", help="Import after download")
    p_cd.add_argument("--slot", type=int, default=0, help="Target slot 1..30 when importing")
    p_cd.add_argument("--host", default=DEFAULT_PRINTER_HOST, help="Printer IP/hostname")
    p_cd.add_argument("--write-printer", action="store_true", help="Also write to printer slot")
    p_cd.add_argument(
        "--custom-media-json",
        default=str(_ROOT / "custom_media.json"),
        help="Path to custom_media.json",
    )
    p_cd.add_argument("--regenerate-ppd", action="store_true", help="Regenerate PPDs after import")
    p_cd.set_defaults(func=cmd_cloud_download)

    return p


def main(argv: Optional[List[str]] = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return int(args.func(args))


if __name__ == "__main__":
    sys.exit(main())
