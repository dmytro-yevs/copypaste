#!/usr/bin/env python3
import math
import pathlib
import sys
import tempfile


BLACK_CHANNEL_MAX = 8
# DMY-47: the 1% floor may reject near-black UIs occupying less than 1% of a
# frame; release evidence must show more than a sparse overlay or indicator.
MIN_VISIBLE_CONTENT_FRACTION = 0.01
MIN_VISIBLE_CONTENT_PIXELS = 2


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
            rgba = image.convert("RGBA")
            black = Image.new("RGBA", rgba.size, (0, 0, 0, 255))
            visible_rgb = Image.alpha_composite(black, rgba).convert("RGB")
            extrema = visible_rgb.getextrema()
            if all(low == high for low, high in extrema):
                raise ContentlessPngError(
                    f"screenshot artifact is contentless: uniform RGB frame {extrema}"
                )
            value_histogram = visible_rgb.convert("HSV").getchannel("V").histogram()
            visible_pixels = sum(value_histogram[BLACK_CHANNEL_MAX + 1:])
            total_pixels = image.width * image.height
            required_pixels = max(
                MIN_VISIBLE_CONTENT_PIXELS,
                math.ceil(total_pixels * MIN_VISIBLE_CONTENT_FRACTION),
            )
            if visible_pixels < required_pixels:
                raise ContentlessPngError(
                    "screenshot artifact is contentless: "
                    f"{visible_pixels}/{total_pixels} visible content pixels exceed "
                    f"black threshold {BLACK_CHANNEL_MAX}; require {required_pixels}"
                )
    except ContentlessPngError:
        raise
    except (OSError, SyntaxError, ValueError) as error:
        raise ValueError("screenshot artifact must be a complete decodable PNG") from error


def self_test():
    try:
        from PIL import Image, ImageDraw
    except ModuleNotFoundError:
        raise SystemExit("PNG decoder is unavailable; install requirements-ci.txt") from None

    with tempfile.TemporaryDirectory() as directory:
        root = pathlib.Path(directory)

        good = Image.new("RGB", (2, 2), (220, 38, 38))
        good.putpixel((1, 1), (255, 255, 255))
        good.save(root / "good.png")

        for name, bright_pixels in (("below-floor", 99), ("at-floor", 100)):
            image = Image.new("RGB", (100, 100), "black")
            ImageDraw.Draw(image).rectangle((0, 0, bright_pixels - 1, 0), fill="white")
            image.save(root / f"{name}.png")

        sparse = Image.new("RGB", (100, 100), "black")
        sparse.putpixel((99, 99), (255, 255, 255))
        sparse.save(root / "sparse.png")

        near_black = Image.new("RGB", (100, 100), "black")
        ImageDraw.Draw(near_black).rectangle((0, 0, 99, 0), fill=(8, 8, 8))
        near_black.save(root / "near-black.png")

        hidden = Image.new("RGBA", (100, 100), (255, 0, 0, 0))
        ImageDraw.Draw(hidden).rectangle((50, 0, 99, 99), fill=(0, 255, 0, 0))
        hidden.save(root / "transparent-hidden-rgb.png")

        Image.new("RGB", (100, 100), "black").save(root / "uniform-black.png")
        Image.new("RGB", (100, 100), "white").save(root / "uniform-white.png")

        for name in ("good", "at-floor"):
            try:
                validate_png(root / f"{name}.png")
            except ValueError as error:
                raise SystemExit(f"{name} PNG self-test failed: {error}") from None

        rejected = {
            "below-floor": "99/10000 visible content pixels",
            "sparse": "1/10000 visible content pixels",
            "near-black": "0/10000 visible content pixels",
            "transparent-hidden-rgb": "uniform RGB frame",
            "uniform-black": "uniform RGB frame",
            "uniform-white": "uniform RGB frame",
        }
        for name, expected in rejected.items():
            try:
                validate_png(root / f"{name}.png")
            except ContentlessPngError as error:
                if expected not in str(error):
                    raise SystemExit(f"{name} PNG self-test returned: {error}") from None
            else:
                raise SystemExit(f"{name} PNG self-test was accepted")

    print("PNG evidence validator self-test passed")


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        self_test()
    else:
        try:
            validate_png(pathlib.Path(sys.argv[1]))
        except (IndexError, ValueError) as error:
            raise SystemExit(f"PNG validation failed: {error}") from None
