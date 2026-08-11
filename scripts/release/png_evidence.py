#!/usr/bin/env python3
import math
import pathlib
import sys
import tempfile


MIN_VISIBLE_LUMA = 32
MIN_VISIBLE_CONTENT_FRACTION = 0.01
MIN_VISIBLE_CONTENT_PIXELS = 2
MIN_COHERENT_NEIGHBORS = 3
COHERENT_VALUE_MIN = math.ceil(255 * MIN_COHERENT_NEIGHBORS / 9)


class ContentlessPngError(ValueError):
    pass


def validate_png(path):
    try:
        from PIL import Image, ImageFilter
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
            luma = visible_rgb.convert("L")
            visible_mask = luma.point(lambda pixel: 255 if pixel > MIN_VISIBLE_LUMA else 0)
            visible_pixels = visible_mask.histogram()[255]
            coherent_mask = visible_mask.filter(ImageFilter.BoxBlur(1))
            coherent_pixels = sum(coherent_mask.histogram()[COHERENT_VALUE_MIN:])
            total_pixels = image.width * image.height
            required_pixels = max(
                MIN_VISIBLE_CONTENT_PIXELS,
                math.ceil(total_pixels * MIN_VISIBLE_CONTENT_FRACTION),
            )
            if visible_pixels <= required_pixels or coherent_pixels <= required_pixels:
                raise ContentlessPngError(
                    "screenshot artifact is contentless: "
                    f"{visible_pixels}/{total_pixels} visible pixels and "
                    f"{coherent_pixels}/{total_pixels} locally coherent pixels exceed "
                    f"luma {MIN_VISIBLE_LUMA}; require more than {required_pixels}"
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

        isolated = Image.new("RGB", (100, 100), "black")
        for y in range(5, 100, 10):
            for x in range(5, 100, 10):
                isolated.putpixel((x, y), (255, 255, 255))
        isolated.save(root / "isolated-noise.png")

        near_black = Image.new("RGB", (100, 100), "black")
        for y in range(100):
            for x in range(100):
                value = 8 + ((x + y) % 2)
                near_black.putpixel((x, y), (value, value, value))
        near_black.save(root / "near-black-checker.png")

        ui = Image.new("RGB", (360, 720), (15, 17, 23))
        draw = ImageDraw.Draw(ui)
        draw.rectangle((20, 20, 340, 90), fill=(37, 42, 57))
        draw.rectangle((20, 110, 340, 230), fill=(30, 35, 48))
        draw.rectangle((40, 135, 280, 148), fill=(235, 238, 245))
        draw.rectangle((40, 165, 310, 178), fill=(113, 184, 255))
        draw.rectangle((20, 250, 340, 370), fill=(30, 35, 48))
        draw.rectangle((40, 275, 290, 288), fill=(235, 238, 245))
        draw.rectangle((40, 305, 250, 318), fill=(178, 187, 211))
        ui.save(root / "copypaste-ui.png")

        hidden = Image.new("RGBA", (100, 100), (255, 0, 0, 0))
        ImageDraw.Draw(hidden).rectangle((50, 0, 99, 99), fill=(0, 255, 0, 0))
        hidden.save(root / "transparent-hidden-rgb.png")

        Image.new("RGB", (100, 100), "black").save(root / "uniform-black.png")
        Image.new("RGB", (100, 100), "white").save(root / "uniform-white.png")

        for name in ("good", "copypaste-ui"):
            try:
                validate_png(root / f"{name}.png")
            except ValueError as error:
                raise SystemExit(f"{name} PNG self-test failed: {error}") from None

        rejected = {
            "below-floor": "99/10000 visible pixels",
            "at-floor": "100/10000 visible pixels",
            "isolated-noise": "0/10000 locally coherent pixels",
            "near-black-checker": "0/10000 visible pixels",
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
