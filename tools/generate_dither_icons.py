from pathlib import Path

from PIL import Image, ImageDraw


OUT = Path(__file__).resolve().parents[1] / "assets" / "dither_icons"
OUT.mkdir(parents=True, exist_ok=True)

PATTERNS = [
    "none",
    "diagonal_line",
    "vertical_line",
    "horizontal_line",
    "screen",
    "bayer_2x2",
    "bayer_4x4",
    "bayer_8x8",
    "floyd_steinberg",
    "burkes",
    "atkinson",
]


def save(name: str, draw_pattern) -> None:
    image = Image.new("RGBA", (64, 64), (255, 255, 255, 255))
    draw = ImageDraw.Draw(image)
    draw.rectangle((1, 1, 62, 62), outline=(120, 120, 120), width=2)
    draw_pattern(draw)
    image.save(OUT / f"{name}.png")


def points(draw, values, radius=3):
    for x, y in values:
        draw.ellipse((x - radius, y - radius, x + radius, y + radius), fill=(25, 25, 25, 255))


save("none", lambda draw: None)
save("diagonal_line", lambda draw: draw.line((5, 59, 59, 5), fill=(25, 25, 25, 255), width=5))
save("vertical_line", lambda draw: [draw.line((x, 5, x, 59), fill=(25, 25, 25, 255), width=4) for x in (18, 46)])
save("horizontal_line", lambda draw: [draw.line((5, y, 59, y), fill=(25, 25, 25, 255), width=4) for y in (18, 46)])
save("screen", lambda draw: points(draw, [(18, 18), (46, 46)], 5))
save("bayer_2x2", lambda draw: points(draw, [(20, 20)], 6))
save("bayer_4x4", lambda draw: points(draw, [(12, 12), (36, 12), (24, 24), (48, 24), (12, 40), (36, 40), (24, 52), (48, 52)], 3))
save("bayer_8x8", lambda draw: points(draw, [(9, 9), (25, 13), (41, 9), (57, 13), (17, 29), (33, 35), (49, 29), (57, 45), (9, 49), (25, 57), (41, 49)], 2))
save("floyd_steinberg", lambda draw: points(draw, [(14, 14), (32, 22), (50, 14), (22, 46), (42, 40)], 4))
save("burkes", lambda draw: points(draw, [(11, 14), (26, 14), (48, 20), (18, 45), (39, 52)], 4))
save("atkinson", lambda draw: points(draw, [(16, 14), (38, 14), (27, 31), (50, 42), (14, 51)], 4))
