from common import box, cylinder

# Sizes shared by every model, in meters. Blender axes: y is the direction of
# travel, z is up.
PIECE = 2.0
GAUGE = 1.4
SLEEPER = (2.2, 0.26, 0.16)
SLEEPER_STEP = 0.5
RAIL = (0.22, 0.18)
RAIL_TOP = SLEEPER[2] + RAIL[1]
WHEEL_RADIUS = 0.42
WHEEL_WIDTH = 0.16
AXLE_Z = RAIL_TOP + WHEEL_RADIUS
DECK_Z = AXLE_Z + WHEEL_RADIUS + 0.08


def track(name, y0=0.0, length=PIECE):
    """A straight run of track starting at y0."""
    count = round(length / SLEEPER_STEP)
    for i in range(count):
        y = y0 + SLEEPER_STEP * (i + 0.5)
        box(f"{name}_sleeper{i}", SLEEPER, (0, y, SLEEPER[2] / 2), "wood", grain="x")
    # Each rail is stone blocks of 1 m with a thin joint, like laid masonry.
    blocks = round(length)
    for side in (-1, 1):
        for i in range(blocks):
            y = y0 + i + 0.5
            box(
                f"{name}_rail{side}_{i}",
                (RAIL[0], 0.98, RAIL[1]),
                (side * GAUGE / 2, y, SLEEPER[2] + RAIL[1] / 2),
                "stone",
                grain="y",
            )


def axle(name, y):
    """Two stone wheels on a wooden axle, with a flange on the inner side."""
    cylinder(f"{name}_axle", 0.07, GAUGE + 0.3, (0, y, AXLE_Z), "wood", axis="x")
    for side in (-1, 1):
        x = side * GAUGE / 2
        cylinder(f"{name}_wheel{side}", WHEEL_RADIUS, WHEEL_WIDTH, (x, y, AXLE_Z), "stone", axis="x", segments=16)
        cylinder(
            f"{name}_flange{side}",
            WHEEL_RADIUS + 0.07,
            0.05,
            (x - side * (WHEEL_WIDTH / 2 + 0.025), y, AXLE_Z),
            "stone",
            axis="x",
            segments=16,
        )
        cylinder(f"{name}_hub{side}", 0.11, 0.08, (x + side * (WHEEL_WIDTH / 2 + 0.04), y, AXLE_Z), "iron", axis="x", segments=8)


def frame(name, length, width):
    """Two long beams and cross beams that carry the deck above the axles."""
    z = DECK_Z - 0.12
    for side in (-1, 1):
        box(f"{name}_beam{side}", (0.18, length, 0.2), (side * width / 2, 0, z), "wood", grain="y")
    for end in (-1, 1):
        # Between the long beams, not through them: overlapping faces in one
        # plane flicker in the game.
        box(f"{name}_cross{end}", (width - 0.18, 0.2, 0.2), (0, end * (length / 2 - 0.1), z), "wood")


def coupler(name, y, direction):
    box(f"{name}_coupler", (0.16, 0.4, 0.14), (0, y + direction * 0.2, DECK_Z - 0.12), "wood", grain="y")
    box(f"{name}_ring", (0.2, 0.06, 0.2), (0, y + direction * 0.42, DECK_Z - 0.12), "iron")
