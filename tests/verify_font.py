import sys

from fontTools.ttLib import TTFont


REQUIRED_TAGS = {"head", "name", "cmap", "hhea", "hmtx", "maxp", "glyf", "loca"}


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: verify_font.py <font-path>", file=sys.stderr)
        return 2

    font = TTFont(sys.argv[1])
    present_tags = set(font.reader.keys())
    missing = sorted(REQUIRED_TAGS.difference(present_tags))
    if missing:
        print(f"missing required tables: {', '.join(missing)}", file=sys.stderr)
        return 1

    print("font structure verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
