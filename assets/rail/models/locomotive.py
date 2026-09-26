from common import box, cylinder
from parts import DECK_Z, axle, coupler, frame

LENGTH = 4.4
WIDTH = 1.8
BOILER_RADIUS = 0.58
BOILER_LENGTH = 2.3
BOILER_Y = 0.75
FIREBOX_HEIGHT = 0.5
BOILER_Z = DECK_Z + 0.04 + FIREBOX_HEIGHT + BOILER_RADIUS - 0.1
PLATFORM_Y = -1.45


def build():
    for i, y in enumerate((1.4, 0.0, -1.4)):
        axle(f"axle{i}", y)
    frame("frame", LENGTH, WIDTH - 0.5)
    coupler("back", -LENGTH / 2, -1)

    # The deck under the whole engine, boards across.
    boards = 11
    step = LENGTH / boards
    for i in range(boards):
        y = -LENGTH / 2 + step * (i + 0.5)
        box(f"deck{i}", (WIDTH, step - 0.02, 0.08), (0, y, DECK_Z), "wood", grain="x")

    # Stone firebox under the boiler, with an iron fire door at the back.
    fire_top = DECK_Z + 0.04 + FIREBOX_HEIGHT
    # Its back face 2 cm in front of the end of the boiler, faces in one plane
    # flicker in the game.
    box("firebox", (1.3, BOILER_LENGTH - 0.34, FIREBOX_HEIGHT), (0, BOILER_Y - 0.13, DECK_Z + 0.04 + FIREBOX_HEIGHT / 2), "stone", grain="y")
    box("firedoor", (0.5, 0.05, 0.34), (0, BOILER_Y - BOILER_LENGTH / 2, fire_top - FIREBOX_HEIGHT / 2), "iron")

    # A lying barrel of wooden staves, held by iron bands.
    cylinder("boiler", BOILER_RADIUS, BOILER_LENGTH, (0, BOILER_Y, BOILER_Z), "wood", axis="y", segments=16)
    for i, dy in enumerate((-0.9, -0.3, 0.3, 0.9)):
        cylinder(f"boilerband{i}", BOILER_RADIUS + 0.03, 0.08, (0, BOILER_Y + dy, BOILER_Z), "iron", axis="y", segments=16)
    front = BOILER_Y + BOILER_LENGTH / 2
    cylinder("boilerfront", BOILER_RADIUS - 0.06, 0.06, (0, front + 0.03, BOILER_Z), "wood", axis="y", segments=16)
    cylinder("boilerboss", 0.16, 0.08, (0, front + 0.08, BOILER_Z), "iron", axis="y", segments=8)

    # A tall stone chimney at the front, narrowing to the top, with an iron rim.
    chimney_y = front - 0.4
    base = BOILER_Z + BOILER_RADIUS - 0.08
    box("chimneybase", (0.62, 0.62, 0.3), (0, chimney_y, base + 0.15), "stone", grain="z")
    cylinder("chimney", 0.25, 1.2, (0, chimney_y, base + 0.9), "stone", axis="z", segments=8, radius2=0.2)
    cylinder("chimneyrim", 0.27, 0.1, (0, chimney_y, base + 1.52), "iron", axis="z", segments=8)

    # Buffer beam at the front.
    box("buffer", (WIDTH + 0.1, 0.2, 0.3), (0, LENGTH / 2 + 0.05, DECK_Z - 0.1), "wood")
    for side in (-1, 1):
        cylinder(f"bufferpad{side}", 0.1, 0.12, (side * 0.6, LENGTH / 2 + 0.2, DECK_Z - 0.1), "iron", axis="y", segments=8)

    # Driver platform at the back: a coal bin, a bench seat and side rails.
    bin_y = PLATFORM_Y + 0.75
    for side in (-1, 1):
        box(f"binside{side}", (0.06, 0.6, 0.5), (side * 0.55, bin_y, DECK_Z + 0.29), "wood", grain="y")
    # Between the side boards and a little narrower than the seat, overlapping
    # faces in one plane flicker in the game.
    box("binback", (1.04, 0.06, 0.5), (0, bin_y - 0.3, DECK_Z + 0.29), "wood")
    box("coal", (1.04, 0.54, 0.2), (0, bin_y, DECK_Z + 0.3), "iron")

    seat_y = PLATFORM_Y - 0.45
    box("seat", (1.1, 0.4, 0.08), (0, seat_y, DECK_Z + 0.48), "wood")
    box("seatback", (1.06, 0.06, 0.5), (0, seat_y - 0.2, DECK_Z + 0.75), "wood")
    for side in (-1, 1):
        box(f"seatleg{side}", (0.08, 0.36, 0.44), (side * 0.5, seat_y, DECK_Z + 0.26), "wood", grain="z")

    for side in (-1, 1):
        x = side * (WIDTH / 2 - 0.05)
        for i, y in enumerate((PLATFORM_Y - 0.65, PLATFORM_Y + 0.35)):
            box(f"railpost{side}{i}", (0.08, 0.08, 0.9), (x, y, DECK_Z + 0.49), "wood", grain="z")
        box(f"railbar{side}", (0.07, 1.1, 0.07), (x, PLATFORM_Y - 0.15, DECK_Z + 0.9), "wood", grain="y")
