#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
LZ_RS = ROOT / "crates" / "fonttool-mtx" / "src" / "lz.rs"
INPUT_TTF = ROOT / "build" / "pptx_case7" / "font1.decoded.ttf"
OUT_DIR = ROOT / "build" / "hash_chain_bench"
HASH_CHAIN_PATTERN = re.compile(r"^const MAX_HASH_CHAIN: usize = \d+;$", re.MULTILINE)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Benchmark case7 EOT size/runtime across MAX_HASH_CHAIN values."
    )
    parser.add_argument(
        "--values",
        default="64,96,128,160,192,256",
        help="Comma-separated MAX_HASH_CHAIN values to try.",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=120.0,
        help="Per-encode timeout in seconds.",
    )
    return parser.parse_args()


def run(cmd: list[str], timeout: float | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=ROOT,
        text=True,
        capture_output=True,
        timeout=timeout,
        check=False,
    )


def patch_hash_chain(source: str, value: int) -> str:
    replacement = f"const MAX_HASH_CHAIN: usize = {value};"
    patched, count = HASH_CHAIN_PATTERN.subn(replacement, source, count=1)
    if count != 1:
        raise RuntimeError("failed to locate MAX_HASH_CHAIN definition")
    return patched


def inspect_eot(path: Path) -> dict[str, int]:
    data = bytearray(path.read_bytes())
    eot_size = int.from_bytes(data[0:4], "little")
    font_data_size = int.from_bytes(data[4:8], "little")
    flags = int.from_bytes(data[8:12], "little")
    header_length = eot_size - font_data_size
    payload = data[header_length : header_length + font_data_size]
    if flags & 0x1000_0000:
        for index in range(len(payload)):
            payload[index] ^= 0x50
    return {
        "file_size": len(data),
        "header_length": header_length,
        "font_data_size": font_data_size,
        "copy_dist": int.from_bytes(payload[1:4], "big"),
        "offset2": int.from_bytes(payload[4:7], "big"),
        "offset3": int.from_bytes(payload[7:10], "big"),
        "block1_len": int.from_bytes(payload[4:7], "big") - 10,
    }


def print_result(value: int, status: str, elapsed: float | None, metrics: dict[str, int] | None) -> None:
    duration = "-" if elapsed is None else f"{elapsed:.2f}s"
    if metrics is None:
        print(f"{value:>3}  {status:<8}  {duration:<8}", flush=True)
        return
    print(
        f"{value:>3}  {status:<8}  {duration:<8}  "
        f"file={metrics['file_size']}  font_data={metrics['font_data_size']}  "
        f"block1={metrics['block1_len']}  copy_dist={metrics['copy_dist']}",
        flush=True,
    )


def main() -> int:
    args = parse_args()
    values = [int(item.strip()) for item in args.values.split(",") if item.strip()]
    if not INPUT_TTF.exists():
        print(f"missing case7 input at {INPUT_TTF}", file=sys.stderr)
        return 2

    original = LZ_RS.read_text()
    OUT_DIR.mkdir(parents=True, exist_ok=True)

    print("MAX  status    elapsed   details", flush=True)
    try:
        for value in values:
            LZ_RS.write_text(patch_hash_chain(original, value))

            build = run(["cargo", "build", "-q", "-p", "fonttool-cli"])
            if build.returncode != 0:
                print_result(value, "build-fail", None, None)
                sys.stderr.write(build.stderr)
                return build.returncode

            output = OUT_DIR / f"case7-hash-{value}.eot"
            if output.exists():
                output.unlink()

            start = time.perf_counter()
            try:
                encode = run(
                    [
                        str(ROOT / "target" / "debug" / "fonttool"),
                        "encode",
                        str(INPUT_TTF),
                        str(output),
                    ],
                    timeout=args.timeout,
                )
            except subprocess.TimeoutExpired:
                print_result(value, "timeout", args.timeout, None)
                continue
            elapsed = time.perf_counter() - start

            if encode.returncode != 0:
                print_result(value, "enc-fail", elapsed, None)
                sys.stderr.write(encode.stderr)
                return encode.returncode

            metrics = inspect_eot(output)
            print_result(value, "ok", elapsed, metrics)
    finally:
        LZ_RS.write_text(original)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
