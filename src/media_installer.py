#!/usr/bin/env python3
"""
media_installer.py - Linux implementation of Epson Media Installer for the
                     Epson SureColor SC-P900 and SC-P700.

Provides:
  1. SNMPv1 Private-MIB tunnel (OID 1.3.6.1.4.1.1248.1.2.2.44.1.1.2.1, Type 1-raw)
     to read, write, and clear the printer's 30 custom media NVRAM slots (1..30):
       - ReadBackup (30-slot directory in 1 packet)
       - ReadBank(1..30) (all TLV mechanical & identification tags)
       - WriteBank(1..30) (2-step TLV NVRAM registration)
       - ClearBank(1..30) (slot deletion via m2 = slot + 0x80)
  2. .emx package parser & importer (.xml + .cpb + .icc ZIP container).
  3. Synchronization with `custom_media.json` and ICC profile directories for
     seamless integration with `rastertop900escpr`, `generate_ppd.py`, and LumaPrint.
  4. Epson LFP Cloud Content Server client (https://support.epson.biz/lfp/cms/v2/)
     to list and download official `.emx` media definitions.

SPDX-License-Identifier: GPL-3.0-or-later
"""

from __future__ import annotations

import configparser
import io
import json
import pathlib
import random
import re
import socket
import struct
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
import zipfile
from dataclasses import asdict, dataclass
from typing import Any, Dict, List, Optional, Tuple

from escpr_protocol import MEDIA_BY_KEY, MEDIA_SPECS, MediaSpec


# ---------------------------------------------------------------------------
# Constants & Lookup Tables
# ---------------------------------------------------------------------------

# Base OID for Type 1-raw (Mode 3 in CEbapiCommand / CommandSpecP900):
# 1.3.6.1.4.1.1248.1.2.2.44.1.1.2.1
BASE_OID_TYPE1_BER = bytes([
    0x2B, 0x06, 0x01, 0x04, 0x01, 0x89, 0x60,
    0x01, 0x02, 0x02, 0x2C, 0x01, 0x01, 0x02, 0x01,
])

MEDIA_CATEGORIES: Dict[int, str] = {
    1: "Photo Paper",
    2: "Proofing Paper",
    3: "Matte Paper",
    4: "Fine Art Paper",
    5: "Canvas",
    6: "Plain Paper",
    7: "Post Card",
    254: "Others",
    255: "Unregistered",
}

# Map hardware base MediaTypeID (sn_id / LegacyBaseMedia) to driver MediaType key
SN_ID_TO_BASE_KEY: Dict[int, str] = {m.sn_id: m.key for m in MEDIA_SPECS if m.user_media_id == 0}

CMS_BASE_URL = "https://support.epson.biz/lfp/cms/v2/"


# ---------------------------------------------------------------------------
# Data Structures
# ---------------------------------------------------------------------------

@dataclass
class PrinterMediaSlot:
    slot: int
    registered: bool = False
    media_name: str = ""
    user_media_id: int = 0          # Tag 0x24: 32-bit MediaID / MA_ID sent in ESC q setl
    base_sn_id: int = 25            # Tag 0x02: LegacyBaseMedia (e.g. 13 Luster, 25 VelvetFineArt, 174 Baryta)
    media_category: int = 4         # Tag 0x25: 1=Photo, 2=Proofing, 3=Matte, 4=FineArt, 5=Canvas, 6=Plain
    platen_gap: int = 21            # Tag 0x03: PlatenGap (9=Auto/0.9, 12=1.2, 15=1.5/1.6, 21=2.1)
    thickness_01mm: int = 48        # Tag 0x19: PaperThickness in 0.01 mm units (48 = 0.48 mm)
    drying_time: int = 0            # Tag 0x04: DryTime per pass (0..100)
    paper_width_check: int = 1      # Tag 0x22: MeasurementPaperWidth (1=On, 0=Off)
    cut_sheet_feed_offset: int = 0  # Tag 0x28[0..2]: int16 LE
    roll_paper_feed_offset: int = 0 # Tag 0x28[2..4]: int16 LE
    margin_sets: int = 0            # Tag 0x2B: NumberOfMarginSet (0)

    @property
    def thickness_mm(self) -> float:
        return round(self.thickness_01mm / 100.0, 2)

    @property
    def base_media_key(self) -> str:
        return SN_ID_TO_BASE_KEY.get(self.base_sn_id, "VelvetFineArt")

    @property
    def category_name(self) -> str:
        return MEDIA_CATEGORIES.get(self.media_category, f"Category({self.media_category})")

    def suggested_driver_key(self) -> str:
        trans = (
            self.media_name.replace("ä", "ae")
            .replace("ö", "oe")
            .replace("ü", "ue")
            .replace("Ä", "Ae")
            .replace("Ö", "Oe")
            .replace("Ü", "Ue")
            .replace("ß", "ss")
        )
        slug = re.sub(r"[^A-Za-z0-9]+", "_", trans).strip("_")
        if not slug:
            slug = f"Slot{self.slot:02d}"
        return f"Slot{self.slot:02d}_{slug}"[:32].rstrip("_")


@dataclass
class EmxPackage:
    source_path: str
    model_name: str
    series_name: str
    user_media_id: int
    vendor: str
    media_name: str
    media_full_name: str
    base_sn_id: int
    mac_ppd_id: int
    win_drv_id: int
    media_category: int
    platen_gap: int
    thickness_01mm: int
    drying_time: int
    paper_width_check: int
    cut_sheet_feed_offset: int
    roll_paper_feed_offset: int
    ink_density: int                # 100 = 0% PrintDuty; 50..150 -> PrintDuty -50..+50
    icc_profile_name: str
    icc_profile_bytes: bytes

    @property
    def print_duty(self) -> int:
        return max(-50, min(50, int(self.ink_density) - 100))

    @property
    def thickness_mm(self) -> float:
        return round(self.thickness_01mm / 100.0, 2)

    @property
    def base_media_key(self) -> str:
        return SN_ID_TO_BASE_KEY.get(self.base_sn_id, "VelvetFineArt")


# ---------------------------------------------------------------------------
# ASN.1 / BER & SNMPv1 Type 1-raw Codec
# ---------------------------------------------------------------------------

def _ber_len(length: int) -> bytes:
    if length < 0x80:
        return bytes([length])
    if length <= 0xFF:
        return bytes([0x81, length])
    return bytes([0x82, (length >> 8) & 0xFF, length & 0xFF])


def _ber_tlv(tag: int, val: bytes) -> bytes:
    return bytes([tag]) + _ber_len(len(val)) + val


def _ber_read_tlv(buf: bytes, pos: int) -> Tuple[int, bytes, int]:
    tag = buf[pos]
    pos += 1
    b0 = buf[pos]
    pos += 1
    if (b0 & 0x80) == 0:
        length = b0
    else:
        nbytes = b0 & 0x7F
        length = 0
        for _ in range(nbytes):
            length = (length << 8) | buf[pos]
            pos += 1
    return tag, buf[pos : pos + length], pos + length


def _encode_oid_subids(raw_bytes: bytes) -> bytes:
    out = bytearray()
    for b in raw_bytes:
        if b < 0x80:
            out.append(b)
        else:
            out.append(0x81)
            out.append(b - 0x80)
    return bytes(out)


def _build_snmp_get_type1_raw(cmd_bytes: bytes, community: str = "public", req_id: int = 1) -> bytes:
    oid_bytes = BASE_OID_TYPE1_BER + _encode_oid_subids(cmd_bytes)
    varbind = _ber_tlv(0x30, _ber_tlv(0x06, oid_bytes) + _ber_tlv(0x05, b""))
    varbind_list = _ber_tlv(0x30, varbind)
    req_id_bytes = req_id.to_bytes(4, "big", signed=True).lstrip(b"\x00") or b"\x01"
    if req_id_bytes[0] & 0x80:
        req_id_bytes = b"\x00" + req_id_bytes
    pdu = _ber_tlv(
        0xA0,  # GetRequest-PDU
        _ber_tlv(0x02, req_id_bytes)
        + _ber_tlv(0x02, b"\x00")
        + _ber_tlv(0x02, b"\x00")
        + varbind_list,
    )
    msg = _ber_tlv(
        0x30,
        _ber_tlv(0x02, b"\x00")  # version = 0 (SNMPv1)
        + _ber_tlv(0x04, community.encode("ascii"))
        + pdu,
    )
    return msg


def _parse_snmp_response(resp: bytes) -> Tuple[int, int, bytes]:
    _, msg_val, _ = _ber_read_tlv(resp, 0)
    pos = 0
    _, _ver, pos = _ber_read_tlv(msg_val, pos)
    _, _comm, pos = _ber_read_tlv(msg_val, pos)
    _, pdu_val, _ = _ber_read_tlv(msg_val, pos)
    ppos = 0
    _, _req_id, ppos = _ber_read_tlv(pdu_val, ppos)
    _, err_stat_b, ppos = _ber_read_tlv(pdu_val, ppos)
    _, err_idx_b, ppos = _ber_read_tlv(pdu_val, ppos)
    err_stat = int.from_bytes(err_stat_b, "big", signed=True)
    err_idx = int.from_bytes(err_idx_b, "big", signed=True)
    _, vbl_val, _ = _ber_read_tlv(pdu_val, ppos)
    _, vb_val, _ = _ber_read_tlv(vbl_val, 0)
    vpos = 0
    _, _oid_b, vpos = _ber_read_tlv(vb_val, vpos)
    _, val_b, _ = _ber_read_tlv(vb_val, vpos)
    return err_stat, err_idx, val_b


def send_ex_command_snmp(
    host: str,
    cmd_bytes: bytes,
    port: int = 161,
    community: str = "public",
    timeout: float = 3.0,
) -> bytes:
    """
    Send a binary 'ex' REMOTE1 command to the SC-P900/SC-P700 over SNMPv1
    Type 1-raw and return the raw BDC PS response bytes.
    """
    pkt = _build_snmp_get_type1_raw(cmd_bytes, community=community)
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.settimeout(timeout)
        sock.sendto(pkt, (host, port))
        resp, _ = sock.recvfrom(65535)
    err_stat, err_idx, val_b = _parse_snmp_response(resp)
    if err_stat != 0:
        raise RuntimeError(f"SNMP error status={err_stat} index={err_idx} from {host}:{port}")
    return val_b


def _extract_ex_payload(bdc_reply: bytes) -> Tuple[int, int, int, bytes]:
    """
    Parse a '\x00@BDC PS\r\nex:<res_code:1><m2:1><m3:1><len:2 LE><payload>;\x0c'
    response and return (res_code, m2, m3, payload).
    """
    idx = bdc_reply.find(b"ex:")
    if idx < 0 or len(bdc_reply) < idx + 8:
        raise ValueError(f"Invalid BDC PS ex: reply: {bdc_reply!r}")
    hdr = bdc_reply[idx + 3 : idx + 8]
    res_code = hdr[0]
    m2 = hdr[1]
    m3 = hdr[2]
    payload_len = struct.unpack_from("<H", hdr, 3)[0]
    payload = bdc_reply[idx + 8 : idx + 8 + payload_len]
    return res_code, m2, m3, payload


# ---------------------------------------------------------------------------
# Printer NVRAM Slot Operations (ReadBackup, ReadBank, WriteBank, ClearBank)
# ---------------------------------------------------------------------------

def read_backup(host: str, port: int = 161, timeout: float = 3.0) -> List[Dict[str, Any]]:
    """
    Execute ReadBackup (`65 78 08 00 01 00 00 00 1b 00 00 c0`) to read the compact
    directory of all 30 custom media slots in a single SNMP packet.
    """
    cmd = bytes.fromhex("65780800010000001b0000c0")
    reply = send_ex_command_snmp(host, cmd, port=port, timeout=timeout)
    res_code, _m2, _m3, payload = _extract_ex_payload(reply)
    if res_code != 0:
        raise RuntimeError(f"ReadBackup returned error code 0x{res_code:02x}")
    entries: List[Dict[str, Any]] = []
    for i in range(0, len(payload), 9):
        chunk = payload[i : i + 9]
        if len(chunk) < 9:
            break
        slot_no = chunk[0]
        base_sn_id = chunk[1]
        media_id = struct.unpack_from("<I", chunk, 2)[0]
        media_cat = chunk[6]
        unk_flag = chunk[7]
        registered = bool(chunk[8])
        entries.append({
            "slot": slot_no,
            "registered": registered,
            "base_sn_id": base_sn_id,
            "user_media_id": media_id,
            "media_category": media_cat,
            "flag": unk_flag,
        })
    return entries


def _decode_slot_tlv(slot: int, payload: bytes) -> PrinterMediaSlot:
    info = PrinterMediaSlot(slot=slot)
    pos = 0
    while pos + 2 <= len(payload):
        tag = payload[pos]
        ln = payload[pos + 1]
        val = payload[pos + 2 : pos + 2 + ln]
        pos += 2 + ln
        if tag == 0x02 and ln == 1:
            info.base_sn_id = val[0]
        elif tag == 0x03 and ln == 1:
            info.platen_gap = val[0]
        elif tag == 0x04 and ln == 1:
            info.drying_time = val[0]
        elif tag == 0x19 and ln == 2:
            info.thickness_01mm = struct.unpack("<H", val)[0]
        elif tag == 0x22 and ln == 1:
            info.paper_width_check = val[0]
        elif tag == 0x23:
            info.media_name = val.decode("utf-8", errors="replace").rstrip("\x00")
        elif tag == 0x24 and ln == 4:
            info.user_media_id = struct.unpack("<I", val)[0]
        elif tag == 0x25 and ln == 1:
            info.media_category = val[0]
        elif tag == 0x26 and ln == 1:
            info.registered = bool(val[0])
        elif tag == 0x28 and ln == 4:
            cut_off, roll_off = struct.unpack("<hh", val)
            info.cut_sheet_feed_offset = cut_off
            info.roll_paper_feed_offset = roll_off
        elif tag == 0x2B and ln == 2:
            info.margin_sets = struct.unpack("<H", val)[0]
    return info


def read_bank(host: str, slot: int, port: int = 161, timeout: float = 3.0) -> PrinterMediaSlot:
    """
    Execute ReadBank(slot) (`65 78 08 00 01 00 00 00 1b 00 <slot> 00`) for slot 1..30.
    """
    if not (1 <= slot <= 30):
        raise ValueError(f"Slot must be in 1..30 (got {slot})")
    cmd = bytes([0x65, 0x78, 0x08, 0x00, 0x01, 0x00, 0x00, 0x00, 0x1B, 0x00, slot, 0x00])
    reply = send_ex_command_snmp(host, cmd, port=port, timeout=timeout)
    res_code, _m2, _m3, payload = _extract_ex_payload(reply)
    if res_code != 0:
        raise RuntimeError(f"ReadBank(slot={slot}) returned error code 0x{res_code:02x}")
    return _decode_slot_tlv(slot, payload)


def read_all_slots(
    host: str,
    port: int = 161,
    only_registered: bool = False,
    timeout: float = 3.0,
) -> List[PrinterMediaSlot]:
    """
    Read all 30 custom media slots from the SC-P900/SC-P700.
    If `only_registered=True`, uses ReadBackup first to query only occupied slots.
    """
    backup = read_backup(host, port=port, timeout=timeout)
    slots: List[PrinterMediaSlot] = []
    for entry in backup:
        slot_no = entry["slot"]
        if only_registered and not entry["registered"]:
            continue
        if entry["registered"]:
            slots.append(read_bank(host, slot_no, port=port, timeout=timeout))
        else:
            slots.append(
                PrinterMediaSlot(
                    slot=slot_no,
                    registered=False,
                    base_sn_id=entry["base_sn_id"],
                    user_media_id=entry["user_media_id"],
                    media_category=entry["media_category"],
                )
            )
    return slots


def _build_write_bank_cmd(slot: int, tlv_payload: bytes) -> bytes:
    total_len = len(tlv_payload) + 8
    return (
        bytes([0x65, 0x78])
        + struct.pack("<H", total_len)
        + bytes([0x00, 0x00, 0x00, 0x00, 0x1B, 0x00, slot & 0xFF, 0x80])
        + tlv_payload
    )


def write_bank(host: str, slot_data: PrinterMediaSlot, port: int = 161, timeout: float = 3.0) -> None:
    """
    Write a custom media configuration into printer NVRAM slot 1..30 using the
    exact two-packet sequence from `TimbreSequenceP900::SetMedia` (`0x100ccdd0..0x100cd780`).
    """
    if not (1 <= slot_data.slot <= 30):
        raise ValueError(f"Slot must be in 1..30 (got {slot_data.slot})")

    name_bytes = slot_data.media_name.encode("utf-8")[:60]
    media_id = slot_data.user_media_id
    if media_id <= 0:
        media_id = 900000000 + random.randint(10000000, 99999999)
        slot_data.user_media_id = media_id

    # Packet 1: Tags 0x02, 0x03, 0x04, 0x19, 0x22, 0x23, 0x24, 0x25, 0x26, 0x28
    tlv1 = bytearray()
    tlv1 += bytes([0x02, 0x01, slot_data.base_sn_id & 0xFF])
    tlv1 += bytes([0x03, 0x01, slot_data.platen_gap & 0xFF])
    tlv1 += bytes([0x04, 0x01, max(0, min(100, slot_data.drying_time)) & 0xFF])
    tlv1 += bytes([0x19, 0x02]) + struct.pack("<H", max(1, min(1000, slot_data.thickness_01mm)))
    tlv1 += bytes([0x22, 0x01, 1 if slot_data.paper_width_check else 0])
    tlv1 += bytes([0x23, len(name_bytes)]) + name_bytes
    tlv1 += bytes([0x24, 0x04]) + struct.pack("<I", media_id & 0xFFFFFFFF)
    tlv1 += bytes([0x25, 0x01, slot_data.media_category & 0xFF])
    tlv1 += bytes([0x26, 0x01, 0x01])  # RegisteredFlag = 1
    tlv1 += bytes([0x28, 0x04]) + struct.pack(
        "<hh",
        slot_data.cut_sheet_feed_offset,
        slot_data.roll_paper_feed_offset,
    )

    cmd1 = _build_write_bank_cmd(slot_data.slot, bytes(tlv1))
    rep1 = send_ex_command_snmp(host, cmd1, port=port, timeout=timeout)
    res1, _, _, _ = _extract_ex_payload(rep1)
    if res1 != 0:
        raise RuntimeError(f"WriteBank packet 1 on slot {slot_data.slot} failed with code 0x{res1:02x}")

    # Packet 2: Tag 0x2B (NumberOfMarginSet = 0)
    tlv2 = bytes([0x2B, 0x02]) + struct.pack("<H", slot_data.margin_sets & 0xFFFF)
    cmd2 = _build_write_bank_cmd(slot_data.slot, tlv2)
    rep2 = send_ex_command_snmp(host, cmd2, port=port, timeout=timeout)
    res2, _, _, _ = _extract_ex_payload(rep2)
    if res2 != 0:
        raise RuntimeError(f"WriteBank packet 2 on slot {slot_data.slot} failed with code 0x{res2:02x}")


def clear_bank(host: str, slot: int, port: int = 161, timeout: float = 3.0) -> None:
    """
    Unregister / clear custom media slot 1..30 in printer NVRAM.
    Per `TimbreSequenceP900::SetMedia` (`0x100cc497`), m2 is `slot + 0x80`.
    """
    if not (1 <= slot <= 30):
        raise ValueError(f"Slot must be in 1..30 (got {slot})")
    cmd = bytes([
        0x65, 0x78, 0x08, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x1B, 0x00, (slot + 0x80) & 0xFF, 0x80,
    ])
    rep = send_ex_command_snmp(host, cmd, port=port, timeout=timeout)
    res_code, _, _, _ = _extract_ex_payload(rep)
    if res_code != 0:
        raise RuntimeError(f"ClearBank(slot={slot}) failed with code 0x{res_code:02x}")


# ---------------------------------------------------------------------------
# .emx Archive Parser & Extractor
# ---------------------------------------------------------------------------

def parse_emx_archive(emx_path: str | pathlib.Path) -> EmxPackage:
    """
    Parse an Epson `.emx` media definition ZIP package containing `<name>.xml`,
    `<name>.cpb`, and `<profile>.icc`.
    """
    p = pathlib.Path(emx_path)
    with zipfile.ZipFile(p, "r") as zf:
        names = zf.namelist()
        xml_names = [n for n in names if n.lower().endswith(".xml")]
        cpb_names = [n for n in names if n.lower().endswith(".cpb")]
        icc_names = [n for n in names if n.lower().endswith((".icc", ".icm"))]

        if not xml_names or not cpb_names:
            raise ValueError(f"{p} is not a valid .emx archive (missing .xml or .cpb member)")

        xml_bytes = zf.read(xml_names[0])
        cpb_text = zf.read(cpb_names[0]).decode("utf-8", errors="replace")

        root = ET.fromstring(xml_bytes)
        model_name = (root.findtext("Name") or "SC-P900 Series").strip()
        series_name = (root.findtext("Series") or model_name).strip()

        mi = root.find("MediaInfo")
        if mi is None:
            raise ValueError(f"Missing <MediaInfo> in {xml_names[0]}")

        xml_media_id = int((mi.findtext("ID") or "0").strip() or 0)
        xml_vendor = (mi.findtext("Manufacturer") or "EPSON").strip()
        xml_media_name = (mi.findtext("MediaName") or p.stem).strip()
        printer_media_id = int((mi.findtext("PrinterMediaID") or "25").strip() or 25)
        mac_ppd_id = int((mi.findtext("MacDrvMediaID") or "53").strip() or 53)
        win_drv_id = int((mi.findtext("WinDrvMediaID") or "0").strip() or 0)

        ink_density = 100
        icc_name_rgb = ""
        for mode_el in mi.findall("ModeInfo"):
            dens_s = (mode_el.findtext("InkDensity") or "").strip()
            if dens_s:
                try:
                    ink_density = int(dens_s)
                except ValueError:
                    pass
            prof_s = (mode_el.findtext("ICCProfileNameRGB") or mode_el.findtext("ICCProfileName") or "").strip()
            if prof_s and not icc_name_rgb:
                icc_name_rgb = prof_s

        # Parse .cpb INI
        cp = configparser.ConfigParser(interpolation=None)
        cp.read_string(cpb_text)
        sec = "Setting01" if cp.has_section("Setting01") else cp.sections()[-1]

        media_name = cp.get(sec, "MediaName", fallback=xml_media_name).strip()
        media_full_name = cp.get(sec, "MediaFullName", fallback=media_name).strip()
        vendor = cp.get(sec, "Vendor", fallback=xml_vendor).strip()
        user_media_id = cp.getint(sec, "MediaID", fallback=xml_media_id)
        base_sn_id = cp.getint(sec, "LegacyBaseMedia", fallback=printer_media_id)
        media_category = cp.getint(sec, "MediaCategory", fallback=4)
        platen_gap = cp.getint(sec, "PlatenGap", fallback=15)
        thickness_01mm = cp.getint(sec, "PaperThickness", fallback=30)
        drying_time = cp.getint(sec, "DryTime", fallback=0)
        paper_width_check = cp.getint(sec, "MeasurementPaperWidth", fallback=1)
        cut_offset = cp.getint(sec, "CutSheetFeedOffset", fallback=0)
        roll_offset = cp.getint(sec, "RollPaperFeedOffset", fallback=0)

        # Locate ICC profile inside ZIP
        icc_member = None
        if icc_name_rgb and icc_name_rgb in names:
            icc_member = icc_name_rgb
        elif icc_names:
            icc_member = icc_names[0]
            if not icc_name_rgb:
                icc_name_rgb = pathlib.Path(icc_member).name

        icc_bytes = zf.read(icc_member) if icc_member else b""

    return EmxPackage(
        source_path=str(p),
        model_name=model_name,
        series_name=series_name,
        user_media_id=user_media_id,
        vendor=vendor,
        media_name=media_name,
        media_full_name=media_full_name,
        base_sn_id=base_sn_id,
        mac_ppd_id=mac_ppd_id,
        win_drv_id=win_drv_id,
        media_category=media_category,
        platen_gap=platen_gap,
        thickness_01mm=thickness_01mm,
        drying_time=drying_time,
        paper_width_check=paper_width_check,
        cut_sheet_feed_offset=cut_offset,
        roll_paper_feed_offset=roll_offset,
        ink_density=ink_density,
        icc_profile_name=icc_name_rgb,
        icc_profile_bytes=icc_bytes,
    )


# ---------------------------------------------------------------------------
# Synchronization with custom_media.json & ICC Profile Directories
# ---------------------------------------------------------------------------

def default_custom_media_json_path() -> pathlib.Path:
    return pathlib.Path(__file__).resolve().parent.parent / "custom_media.json"


def default_icc_output_dirs() -> List[pathlib.Path]:
    repo_icc = pathlib.Path(__file__).resolve().parent.parent / "icc"
    user_icc = pathlib.Path.home() / ".local" / "share" / "color" / "icc"
    sys_icc = pathlib.Path("/usr/share/color/icc/epson-scp900")
    return [repo_icc, user_icc, sys_icc]


def slot_to_custom_media_entry(
    slot_info: PrinterMediaSlot,
    existing: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    existing = existing or {}
    base_key = slot_info.base_media_key
    base_spec: MediaSpec = MEDIA_BY_KEY.get(base_key.lower(), MEDIA_BY_KEY["velvetfineart"])

    prev_key = str(existing.get("key") or "")
    if not prev_key or len(prev_key) > 32 or prev_key.startswith("CustomSlot"):
        key = slot_info.suggested_driver_key()
    else:
        key = prev_key
    icc_prof = existing.get("icc_profile") or base_spec.icc_profile
    print_duty = int(existing.get("print_duty", 0))
    bottom_edge = bool(existing.get("bottom_edge_priority", False))
    rubbing_red = int(existing.get("rubbing_reduction", 0))

    return {
        "slot": slot_info.slot,
        "user_media_id": slot_info.user_media_id,
        "key": key,
        "name": f"{slot_info.media_name} (Slot {slot_info.slot})",
        "media_name": slot_info.media_name,
        "base_media": base_key,
        "base_sn_id": slot_info.base_sn_id,
        "media_category": slot_info.media_category,
        "print_duty": print_duty,
        "platen_gap": slot_info.platen_gap,
        "thickness": slot_info.thickness_01mm,
        "thickness_mm": slot_info.thickness_mm,
        "feed_offset": slot_info.cut_sheet_feed_offset,
        "cut_sheet_feed_offset": slot_info.cut_sheet_feed_offset,
        "roll_paper_feed_offset": slot_info.roll_paper_feed_offset,
        "drying_time": slot_info.drying_time,
        "paper_width_check": slot_info.paper_width_check,
        "bottom_edge_priority": bottom_edge,
        "rubbing_reduction": rubbing_red,
        "icc_profile": icc_prof,
    }


def sync_slots_to_custom_media_json(
    host: str,
    port: int = 161,
    json_path: Optional[pathlib.Path] = None,
) -> Tuple[pathlib.Path, List[Dict[str, Any]]]:
    """
    Read all registered slots (1..30) from the printer at `host` and write/merge
    them into `custom_media.json`.
    """
    target_path = json_path or default_custom_media_json_path()
    existing_entries: List[Dict[str, Any]] = []
    if target_path.is_file():
        try:
            data = json.loads(target_path.read_text(encoding="utf-8"))
            if isinstance(data, list):
                existing_entries = [e for e in data if isinstance(e, dict)]
        except Exception:
            pass

    by_media_id = {int(e.get("user_media_id", 0)): e for e in existing_entries if e.get("user_media_id")}
    by_slot = {int(e.get("slot", 0)): e for e in existing_entries if e.get("slot")}

    registered_slots = read_all_slots(host, port=port, only_registered=True)
    updated: List[Dict[str, Any]] = []
    seen_ids = set()

    for s in registered_slots:
        prev = by_media_id.get(s.user_media_id) or by_slot.get(s.slot)
        entry = slot_to_custom_media_entry(s, existing=prev)
        updated.append(entry)
        seen_ids.add(s.user_media_id)

    # Preserve any unslotted (slot=0) .emx entries already in custom_media.json
    for e in existing_entries:
        if int(e.get("slot", 0) or 0) == 0 and int(e.get("user_media_id", 0) or 0) not in seen_ids:
            updated.append(e)

    target_path.parent.mkdir(parents=True, exist_ok=True)
    target_path.write_text(json.dumps(updated, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return target_path, updated


def import_emx_package(
    emx_path: str | pathlib.Path,
    slot: int = 0,
    json_path: Optional[pathlib.Path] = None,
    printer_host: Optional[str] = None,
    printer_port: int = 161,
) -> Dict[str, Any]:
    """
    Import an `.emx` file:
      1. Extract its `.icc` profile into `icc/` (and `~/.local/share/color/icc/`).
      2. Optionally write the mechanical parameters into `slot` (1..30) on `printer_host`.
      3. Add/update the media entry in `custom_media.json`.
    """
    pkg = parse_emx_archive(emx_path)

    # Save ICC profile to writable ICC directories
    saved_icc_paths: List[str] = []
    if pkg.icc_profile_name and pkg.icc_profile_bytes:
        for d in default_icc_output_dirs():
            try:
                d.mkdir(parents=True, exist_ok=True)
                out_icc = d / pkg.icc_profile_name
                out_icc.write_bytes(pkg.icc_profile_bytes)
                saved_icc_paths.append(str(out_icc))
            except PermissionError:
                continue

    if printer_host and 1 <= slot <= 30:
        slot_obj = PrinterMediaSlot(
            slot=slot,
            registered=True,
            media_name=pkg.media_name[:60],
            user_media_id=pkg.user_media_id,
            base_sn_id=pkg.base_sn_id,
            media_category=pkg.media_category,
            platen_gap=pkg.platen_gap,
            thickness_01mm=pkg.thickness_01mm,
            drying_time=pkg.drying_time,
            paper_width_check=pkg.paper_width_check,
            cut_sheet_feed_offset=pkg.cut_sheet_feed_offset,
            roll_paper_feed_offset=pkg.roll_paper_feed_offset,
        )
        write_bank(printer_host, slot_obj, port=printer_port)

    slug = re.sub(r"[^A-Za-z0-9]+", "_", pkg.media_name).strip("_")
    key = (f"Slot{slot:02d}_{slug}"[:32] if slot else f"EMX_{slug}"[:32]).rstrip("_")
    disp_name = f"{pkg.media_name} (Slot {slot})" if slot else pkg.media_name
    base_key = pkg.base_media_key
    base_spec = MEDIA_BY_KEY.get(base_key.lower(), MEDIA_BY_KEY["velvetfineart"])

    entry: Dict[str, Any] = {
        "slot": slot,
        "user_media_id": pkg.user_media_id,
        "key": key,
        "name": disp_name,
        "media_name": pkg.media_name,
        "vendor": pkg.vendor,
        "base_media": base_key,
        "base_sn_id": pkg.base_sn_id,
        "media_category": pkg.media_category,
        "print_duty": pkg.print_duty,
        "platen_gap": pkg.platen_gap,
        "thickness": pkg.thickness_01mm,
        "thickness_mm": pkg.thickness_mm,
        "feed_offset": pkg.cut_sheet_feed_offset,
        "cut_sheet_feed_offset": pkg.cut_sheet_feed_offset,
        "roll_paper_feed_offset": pkg.roll_paper_feed_offset,
        "drying_time": pkg.drying_time,
        "paper_width_check": pkg.paper_width_check,
        "bottom_edge_priority": False,
        "rubbing_reduction": 0,
        "icc_profile": pkg.icc_profile_name or base_spec.icc_profile,
        "saved_icc_paths": saved_icc_paths,
    }

    target_path = json_path or default_custom_media_json_path()
    entries: List[Dict[str, Any]] = []
    if target_path.is_file():
        try:
            data = json.loads(target_path.read_text(encoding="utf-8"))
            if isinstance(data, list):
                entries = [e for e in data if isinstance(e, dict)]
        except Exception:
            pass

    replaced = False
    for idx, existing in enumerate(entries):
        if (slot and int(existing.get("slot", 0) or 0) == slot) or (
            int(existing.get("user_media_id", 0) or 0) == pkg.user_media_id
        ):
            entries[idx] = {k: v for k, v in entry.items() if k != "saved_icc_paths"}
            replaced = True
            break
    if not replaced:
        entries.append({k: v for k, v in entry.items() if k != "saved_icc_paths"})

    target_path.parent.mkdir(parents=True, exist_ok=True)
    target_path.write_text(json.dumps(entries, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return entry


# ---------------------------------------------------------------------------
# Epson LFP Cloud Content Server API Client
# ---------------------------------------------------------------------------

def _query_cms_jsonp(endpoint: str, query_params: List[Tuple[str, str]], timeout: float = 15.0) -> Dict[str, Any]:
    qs = "callback=__jsonp_callback__&" + "&".join(
        f"{urllib.parse.quote(k)}={urllib.parse.quote(str(v))}" for k, v in query_params
    )
    url = f"{CMS_BASE_URL}{endpoint}?{qs}"
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        text = resp.read().decode("utf-8", errors="replace").strip()
    if "(" in text and text.endswith(")"):
        text = text[text.index("(") + 1 : -1]
    return json.loads(text)


def cloud_list_media(
    printer: str = "scp9xx",
    country: str = "DE",
    region: int = 8,
    locale: str = "de",
    limit: int = 50,
) -> List[Dict[str, Any]]:
    """
    Query Epson's LFP Content Server (`GetMediaList`) for available media and
    `.emx` packages for `scp9xx` (SC-P900) or `scp7xx` (SC-P700).
    """
    params = [
        ("printer", printer),
        ("country", country),
        ("region", str(region)),
        ("locale", locale),
        ("category", "1"),
        ("media_mfr", ""),
        ("media_type", ""),
        ("media_group", ""),
        ("emx_id", ""),
        ("limit", str(limit)),
        ("offset", "0"),
        ("app", "6"),
    ]
    res = _query_cms_jsonp("search", params)
    return res.get("records", [])


def cloud_download_emx(record: Dict[str, Any], out_dir: str | pathlib.Path) -> pathlib.Path:
    """
    Download an `.emx` package from a record returned by `cloud_list_media`.
    """
    raw_url = str(record.get("url") or "")
    if not raw_url:
        raise ValueError(f"Record {record.get('emx_id')} has no download URL")

    parsed = urllib.parse.urlsplit(raw_url)
    q_pairs = urllib.parse.parse_qsl(parsed.query, keep_blank_values=True)
    file_name = dict(q_pairs).get("file") or f"{record.get('emx_id', 'media')}.emx"
    safe_query = urllib.parse.urlencode(q_pairs, quote_via=urllib.parse.quote)
    safe_url = urllib.parse.urlunsplit((parsed.scheme, parsed.netloc, parsed.path, safe_query, parsed.fragment))

    dest_dir = pathlib.Path(out_dir)
    dest_dir.mkdir(parents=True, exist_ok=True)
    dest_path = dest_dir / file_name

    req = urllib.request.Request(safe_url, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(req, timeout=30) as resp:
        dest_path.write_bytes(resp.read())
    return dest_path
