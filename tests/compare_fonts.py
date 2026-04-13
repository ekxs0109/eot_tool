import sys

from fontTools.ttLib import TTFont


ROUNDTRIP_REQUIRED_TAGS = ("head", "name", "cmap", "hhea", "hmtx", "maxp", "glyf", "loca")
ROUNDTRIP_OPTIONAL_PRESERVED_TAGS = ("cvt", "hdmx")
SUBSET_CORE_TAGS = ("head", "cmap", "hhea", "hmtx", "maxp", "glyf", "loca")


def normalized_table_bytes(font: TTFont, tag: str) -> bytes:
    data = bytearray(font.reader[tag])
    if tag == "head" and len(data) >= 12:
        data[8:12] = b"\0\0\0\0"
    return bytes(data)


def require_table(font: TTFont, tag: str, context: str) -> None:
    if tag not in font.reader:
        raise SystemExit(f"missing {context} table: {tag}")

    try:
        font[tag]
    except Exception as exc:
        raise SystemExit(f"failed to parse {context} table {tag}: {exc}") from exc


def require_subset_core_tables(font: TTFont) -> None:
    for tag in SUBSET_CORE_TAGS:
        require_table(font, tag, "subset")


def compare_required_tables(left: TTFont, right: TTFont) -> int:
    for tag in ROUNDTRIP_REQUIRED_TAGS:
        try:
            require_table(left, tag, "left")
            require_table(right, tag, "right")
        except SystemExit as exc:
            print(exc, file=sys.stderr)
            return 1

        left_data = normalized_table_bytes(left, tag)
        right_data = normalized_table_bytes(right, tag)
        if len(left_data) != len(right_data):
            print(
                f"table length mismatch for {tag}: "
                f"{len(left_data)} != {len(right_data)}",
                file=sys.stderr,
            )
            return 1
        if left_data != right_data:
            print(f"table content mismatch for {tag}", file=sys.stderr)
            return 1

    for tag in ROUNDTRIP_OPTIONAL_PRESERVED_TAGS:
        if tag not in left.reader and tag not in right.reader:
            continue

        try:
            require_table(left, tag, "left")
            require_table(right, tag, "right")
        except SystemExit as exc:
            print(exc, file=sys.stderr)
            return 1

        left_data = normalized_table_bytes(left, tag)
        right_data = normalized_table_bytes(right, tag)
        if len(left_data) != len(right_data):
            print(
                f"table length mismatch for {tag}: "
                f"{len(left_data)} != {len(right_data)}",
                file=sys.stderr,
            )
            return 1
        if left_data != right_data:
            print(f"table content mismatch for {tag}", file=sys.stderr)
            return 1

    print("required tables match exactly")
    return 0


def main() -> int:
    if len(sys.argv) == 3 and sys.argv[1] == "--require-subset-core-tables":
        with TTFont(sys.argv[2]) as font:
            require_subset_core_tables(font)
        print("subset core tables verified")
        return 0

    if len(sys.argv) != 3:
        print(
            "usage: compare_fonts.py <left-font> <right-font>\n"
            "   or: compare_fonts.py --require-subset-core-tables <font>",
            file=sys.stderr,
        )
        return 2

    with TTFont(sys.argv[1]) as left, TTFont(sys.argv[2]) as right:
        return compare_required_tables(left, right)


if __name__ == "__main__":
    raise SystemExit(main())
