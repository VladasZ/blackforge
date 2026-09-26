from common import box
from parts import DECK_Z, axle, coupler, frame

LENGTH = 3.2
WIDTH = 1.8
WALL = 0.08
BOARD = 0.22
BOARDS = 3


def build():
    axle("front", 1.0)
    axle("back", -1.0)
    frame("frame", LENGTH, WIDTH - 0.5)
    coupler("front", LENGTH / 2, 1)
    coupler("back", -LENGTH / 2, -1)

    # The floor boards run across, like a vanilla floor.
    floor = 8
    step = LENGTH / floor
    for i in range(floor):
        y = -LENGTH / 2 + step * (i + 0.5)
        box(f"floor{i}", (WIDTH, step - 0.02, 0.08), (0, y, DECK_Z), "wood", grain="x")

    # Walls of horizontal boards with small gaps between them.
    for i in range(BOARDS):
        z = DECK_Z + 0.04 + BOARD / 2 + i * (BOARD + 0.02)
        for side in (-1, 1):
            box(f"side{side}_{i}", (WALL, LENGTH, BOARD), (side * (WIDTH / 2 - WALL / 2), 0, z), "wood", grain="y")
            box(f"end{side}_{i}", (WIDTH - 2 * WALL, WALL, BOARD), (0, side * (LENGTH / 2 - WALL / 2), z), "wood", grain="x")

    height = BOARDS * (BOARD + 0.02) + 0.06
    top = DECK_Z + 0.04 + height / 2
    for sx in (-1, 1):
        for sy in (-1, 0, 1):
            x = sx * (WIDTH / 2 + 0.02)
            y = sy * (LENGTH / 2 - 0.06)
            if sy == 0:
                box(f"band{sx}", (0.03, 0.1, height), (x, 0, top), "iron", grain="z")
            else:
                box(f"post{sx}{sy}", (0.14, 0.14, height + 0.1), (sx * (WIDTH / 2 - 0.02), y, top + 0.05), "wood", grain="z")
                box(f"cap{sx}{sy}", (0.18, 0.18, 0.06), (sx * (WIDTH / 2 - 0.02), y, top + height / 2 + 0.13), "iron")
