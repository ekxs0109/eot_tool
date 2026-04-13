import sys
from dataclasses import dataclass

from fontTools.ttLib import TTFont


@dataclass(frozen=True)
class TableDiff:
    tag: str
    left_len: int
    right_len: int
    first_diff_offset: int
    left_byte: int
    right_byte: int


def _load_table_map(font_path: str) -> dict[str, bytes]:
    with TTFont(font_path) as font:
        return {tag: bytes(font.reader[tag]) for tag in font.reader.keys()}


def _first_diff_offset(left: bytes, right: bytes) -> tuple[int, int, int]:
    limit = min(len(left), len(right))
    for offset in range(limit):
        if left[offset] != right[offset]:
            return offset, left[offset], right[offset]

    if len(left) == len(right):
        return -1, -1, -1

    if len(left) > len(right):
        return limit, left[limit], -1

    return limit, -1, right[limit]


def compare_table_bytes(left_path: str, right_path: str) -> tuple[list[str], list[str], list[TableDiff]]:
    left_tables = _load_table_map(left_path)
    right_tables = _load_table_map(right_path)

    left_tags = set(left_tables.keys())
    right_tags = set(right_tables.keys())

    only_left = sorted(left_tags - right_tags)
    only_right = sorted(right_tags - left_tags)

    diffs: list[TableDiff] = []
    for tag in sorted(left_tags & right_tags):
        left = left_tables[tag]
        right = right_tables[tag]
        if left == right:
            continue
        first_offset, left_byte, right_byte = _first_diff_offset(left, right)
        diffs.append(
            TableDiff(
                tag=tag,
                left_len=len(left),
                right_len=len(right),
                first_diff_offset=first_offset,
                left_byte=left_byte,
                right_byte=right_byte,
            )
        )

    return only_left, only_right, diffs


def _format_byte(value: int) -> str:
    if value < 0:
        return "EOF"
    return f"0x{value:02x}"


def main() -> int:
    if len(sys.argv) != 3:
        print(
            "usage: test_fonttools_parity.py <native-roundtrip.ttf> <fonttools-saved.ttf>",
            file=sys.stderr,
        )
        return 2

    left_path = sys.argv[1]
    right_path = sys.argv[2]
    only_left, only_right, diffs = compare_table_bytes(left_path, right_path)

    print(f"left:  {left_path}")
    print(f"right: {right_path}")

    if only_left:
        print(f"only in left ({len(only_left)}): {', '.join(only_left)}")
    if only_right:
        print(f"only in right ({len(only_right)}): {', '.join(only_right)}")

    if not only_left and not only_right and not diffs:
        print("all shared tables match byte-for-byte")
        return 0

    if diffs:
        print(f"different tables ({len(diffs)}):")
        for diff in diffs:
            print(
                f"  {diff.tag}: len {diff.left_len} vs {diff.right_len}, "
                f"first diff @ {diff.first_diff_offset} "
                f"({_format_byte(diff.left_byte)} vs {_format_byte(diff.right_byte)})"
            )

    return 1


if __name__ == "__main__":
    raise SystemExit(main())
