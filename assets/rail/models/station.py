import math

from common import box, cylinder, rot_y
from parts import DECK_Z, GAUGE

LENGTH = 8.0
WIDTH = 2.6
# The platform edge leaves room for the widest wagon on the track at x = 0.
EDGE = GAUGE / 2 + 0.55
X = EDGE + WIDTH / 2
TOP = DECK_Z + 0.05
ROOF = TOP + 2.6


def build():
    # Deck boards across, on log posts, with an edge beam along the track.
    boards = 20
    step = LENGTH / boards
    for i in range(boards):
        y = -LENGTH / 2 + step * (i + 0.5)
        box(f"deck{i}", (WIDTH, step - 0.02, 0.08), (X, y, TOP - 0.04), "wood", grain="x")
    for side in (-1, 1):
        box(f"edge{side}", (0.16, LENGTH, 0.22), (X + side * (WIDTH / 2 - 0.08), 0, TOP - 0.19), "wood", grain="y")
    for i in range(5):
        y = -LENGTH / 2 + 0.3 + i * (LENGTH - 0.6) / 4
        for side in (-1, 1):
            cylinder(f"leg{side}_{i}", 0.12, TOP - 0.3, (X + side * (WIDTH / 2 - 0.2), y, (TOP - 0.3) / 2), "wood", axis="z", segments=8)

    # Roof posts and a gabled roof along the platform.
    for i in range(3):
        y = -LENGTH / 2 + 0.5 + i * (LENGTH - 1.0) / 2
        for side in (-1, 1):
            cylinder(f"post{side}_{i}", 0.11, ROOF - TOP, (X + side * (WIDTH / 2 - 0.25), y, TOP + (ROOF - TOP) / 2), "wood", axis="z", segments=8)
    for side in (-1, 1):
        box(f"plate{side}", (0.16, LENGTH, 0.18), (X + side * (WIDTH / 2 - 0.25), 0, ROOF + 0.09), "wood", grain="y")

    half = WIDTH / 2 + 0.5
    pitch = 28
    run = half / math.cos(math.radians(pitch))
    rise = half * math.tan(math.radians(pitch))
    # The underside of each roof half rests on the plate at the posts.
    slope = math.tan(math.radians(pitch))
    post = WIDTH / 2 - 0.25
    center = ROOF + 0.18 + 0.05 / math.cos(math.radians(pitch)) + (post - half / 2) * slope
    for side in (-1, 1):
        box(
            f"roof{side}",
            (run, LENGTH + 0.6, 0.1),
            (X + side * half / 2, 0, center),
            "wood",
            grain="y",
            rot=rot_y(side * pitch),
        )
    box("ridge", (0.2, LENGTH + 0.6, 0.2), (X, 0, center + rise / 2), "wood", grain="y")

    # The name sign hangs under the roof edge that faces the track.
    # It sits between two roof posts, so no post runs through it.
    sign_x = X - WIDTH / 2 + 0.25
    sign_y = (LENGTH - 1.0) / 4
    box("sign", (0.08, 2.2, 0.55), (sign_x - 0.02, sign_y, ROOF - 0.55), "wood", grain="y")
    for side in (-1, 1):
        box(f"chain{side}", (0.03, 0.03, 0.3), (sign_x - 0.02, sign_y + side * 0.9, ROOF - 0.15), "iron", grain="z")
