#!/usr/bin/env python3
from __future__ import annotations

import shutil
import subprocess
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ORIG = ROOT / "测试用例 7 (文本组件主题).pptx"
TEST = ROOT / "测试用例 7 (文本组件主题)-test.pptx"
XOR = ROOT / "测试用例 7 (文本组件主题)-xor.pptx"
CASE7_DIR = ROOT / "build" / "pptx_case7"
CASE7_CLEAN = ROOT / "build" / "pptx_case7_clean"
CASE7_XOR = ROOT / "build" / "pptx_case7_xor"
THRESHOLD = 128 * 1024


def rebuild_eot() -> None:
    subprocess.run(
        [
            "cargo",
            "run",
            "-p",
            "fonttool-cli",
            "--bin",
            "fonttool",
            "--",
            "encode",
            str(CASE7_DIR / "font1.decoded.ttf"),
            str(CASE7_DIR / "font1.encoded.eot"),
        ],
        cwd=ROOT,
        check=True,
    )


def write_pptx(source_dir: Path, output_file: Path) -> None:
    if output_file.exists():
        output_file.unlink()
    with zipfile.ZipFile(output_file, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for path in sorted(p for p in source_dir.rglob("*") if p.is_file()):
            zf.write(path, path.relative_to(source_dir).as_posix())


def report_size(name: str, size: int, base: int) -> None:
    delta = size - base
    print(f"{name}: size={size} delta={delta} bytes ({delta / 1024:.1f} KiB)")


def main() -> int:
    rebuild_eot()
    shutil.copyfile(
        CASE7_DIR / "font1.encoded.eot",
        CASE7_CLEAN / "ppt" / "fonts" / "font1.fntdata",
    )

    xor_bytes = bytearray((CASE7_DIR / "font1.encoded.eot").read_bytes())
    xor_bytes[12:16] = (0x10000004).to_bytes(4, "little")
    eot_size = int.from_bytes(xor_bytes[0:4], "little")
    font_data_size = int.from_bytes(xor_bytes[4:8], "little")
    payload_offset = eot_size - font_data_size
    for index in range(payload_offset, len(xor_bytes)):
        xor_bytes[index] ^= 0x50
    (CASE7_XOR / "ppt" / "fonts" / "font1.fntdata").write_bytes(xor_bytes)

    write_pptx(CASE7_CLEAN, TEST)
    write_pptx(CASE7_XOR, XOR)

    base = ORIG.stat().st_size
    test_size = TEST.stat().st_size
    xor_size = XOR.stat().st_size

    report_size("original", base, base)
    report_size("test", test_size, base)
    report_size("xor", xor_size, base)

    if test_size - base > THRESHOLD or xor_size - base > THRESHOLD:
        print("FAIL: regenerated PPTX files exceed the +128 KiB parity budget", file=sys.stderr)
        return 1

    print("PASS: regenerated PPTX files are within the +128 KiB parity budget")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
