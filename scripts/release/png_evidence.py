#!/usr/bin/env python3
import pathlib
import sys


class ContentlessPngError(ValueError):
    pass


def validate_png(path):
    try:
        from PIL import Image
    except ModuleNotFoundError:
        raise ValueError("PNG decoder is unavailable; install requirements-ci.txt") from None
    try:
        with Image.open(path) as image:
            if image.format != "PNG":
                raise ValueError("image format is not PNG")
            image.verify()
        with Image.open(path) as image:
            image.load()
            extrema = image.convert("RGB").getextrema()
            if all(low == high for low, high in extrema):
                raise ContentlessPngError(
                    f"screenshot artifact is contentless: uniform RGB frame {extrema}"
                )
            if max(high for _, high in extrema) <= 8:
                raise ContentlessPngError(
                    f"screenshot artifact is contentless: black RGB frame {extrema}"
                )
    except ContentlessPngError:
        raise
    except (OSError, SyntaxError, ValueError) as error:
        raise ValueError("screenshot artifact must be a complete decodable PNG") from error


if __name__ == "__main__":
    try:
        validate_png(pathlib.Path(sys.argv[1]))
    except (IndexError, ValueError) as error:
        raise SystemExit(f"PNG validation failed: {error}") from None
