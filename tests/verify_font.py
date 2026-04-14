import sys
from pathlib import Path


def _ensure_fonttools_available() -> None:
    try:
        from fontTools.ttLib import TTFont as _  # noqa: F401
        return
    except ImportError as exc:
        repo_python = Path(__file__).resolve().parents[1] / "build" / "venv" / "bin" / "python"
        in_virtualenv = sys.prefix != getattr(sys, "base_prefix", sys.prefix)
        if repo_python.exists() and not in_virtualenv:
            raise SystemExit(
                __import__("subprocess").call([str(repo_python), __file__, *sys.argv[1:]])
            ) from exc

        print(
            "fontTools is required; install it with "
            "`python3 -m pip install -r tests/requirements.txt` "
            "or create the repo venv with `python3 -m venv build/venv` first",
            file=sys.stderr,
        )
        raise SystemExit(2) from exc


_ensure_fonttools_available()

from fontTools.ttLib import TTFont


REQUIRED_TAGS = {"head", "name", "cmap", "hhea", "hmtx", "maxp", "glyf", "loca"}


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: verify_font.py <font-path>", file=sys.stderr)
        return 2

    try:
        with TTFont(sys.argv[1]) as font:
            present_tags = set(font.reader.keys())
            missing = sorted(REQUIRED_TAGS.difference(present_tags))
    except FileNotFoundError:
        print(f"font file not found: {sys.argv[1]}", file=sys.stderr)
        return 2
    except Exception as exc:  # pragma: no cover - fontTools exception surface varies.
        print(f"failed to read font: {exc}", file=sys.stderr)
        return 1

    if missing:
        print(f"missing required tables: {', '.join(missing)}", file=sys.stderr)
        return 1

    print("font structure verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
