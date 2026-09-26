"""Every track piece, built along a path of sleepers and stone rail blocks.

A path starts at the origin heading along +y. A curve turns right, toward +x,
and turns left when it is placed the other way round. Every curve ends on a
multiple of 22.5 degrees, the rotation step of the hammer, so the next piece
lines up. Every track end is a snap point.
"""

import math

from common import LINES, box, rot_x, rot_z, snap
from parts import GAUGE, RAIL, SLEEPER, SLEEPER_STEP

RAIL_BLOCK = 1.0
LINE_STEP = 0.25


class Straight:
    def __init__(self, length, rise=0.0):
        self.length = math.hypot(length, rise)
        self.pitch = math.atan2(rise, length)

    def at(self, s):
        return (0.0, s * math.cos(self.pitch), s * math.sin(self.pitch)), 0.0, self.pitch


class Ease:
    """A slope whose steepness changes evenly from one end to the other, so a
    train tips over gradually. Grades are rise over run."""

    def __init__(self, run, grade0, grade1, samples=64):
        self.table = []
        s = 0.0
        last = (0.0, 0.0)
        for i in range(samples + 1):
            x = run * i / samples
            z = x * grade0 + (grade1 - grade0) * x * x / (2 * run)
            if i:
                s += math.hypot(x - last[0], z - last[1])
            self.table.append((s, x, z))
            last = (x, z)
        self.length = s
        self.run = run
        self.grades = (grade0, grade1)

    def at(self, s):
        for (s0, x0, z0), (s1, x1, z1) in zip(self.table, self.table[1:]):
            if s <= s1 or (s1, x1, z1) == self.table[-1]:
                t = 0.0 if s1 == s0 else min(max((s - s0) / (s1 - s0), 0.0), 1.0)
                x = x0 + t * (x1 - x0)
                grade = self.grades[0] + (self.grades[1] - self.grades[0]) * x / self.run
                return (0.0, x, z0 + t * (z1 - z0)), 0.0, math.atan(grade)
        return (0.0, self.run, 0.0), 0.0, 0.0


class Curve:
    def __init__(self, radius, degrees):
        self.radius = radius
        self.length = radius * math.radians(degrees)

    def at(self, s):
        a = s / self.radius
        return (self.radius * (1 - math.cos(a)), self.radius * math.sin(a), 0.0), a, 0.0


def _turn(heading, pitch):
    return rot_z(-math.degrees(heading)) @ rot_x(math.degrees(pitch))


def _offset(point, heading, side):
    x, y, z = point
    d = side * GAUGE / 2
    return (x + d * math.cos(heading), y - d * math.sin(heading), z)


def run(name, path, mirror=False):
    """Sleepers and both rails along a path, with a snap point at each end."""
    flip = -1 if mirror else 1

    def at(s):
        (x, y, z), heading, pitch = path.at(s)
        return (flip * x, y, z), flip * heading, pitch

    count = max(1, round(path.length / SLEEPER_STEP))
    step = path.length / count
    for i in range(count):
        point, heading, pitch = at(step * (i + 0.5))
        centre = (point[0], point[1], point[2] + SLEEPER[2] / 2)
        box(f"{name}_sleeper{i}", SLEEPER, centre, "wood", grain="x", rot=_turn(heading, pitch))

    blocks = max(1, round(path.length / RAIL_BLOCK))
    for side in (-1, 1):
        for i in range(blocks):
            a = _offset(*at(path.length * i / blocks)[:2], side)
            b = _offset(*at(path.length * (i + 1) / blocks)[:2], side)
            dx, dy, dz = b[0] - a[0], b[1] - a[1], b[2] - a[2]
            flat = math.hypot(dx, dy)
            heading = math.atan2(dx, dy)
            pitch = math.atan2(dz, flat)
            centre = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2 + SLEEPER[2] + RAIL[1] / 2)
            size = (RAIL[0], math.hypot(flat, dz) - 0.02, RAIL[1])
            box(f"{name}_rail{side}_{i}", size, centre, "stone", grain="y", rot=_turn(heading, pitch))

    snap(at(0)[0])
    snap(at(path.length)[0])
    samples = max(2, math.ceil(path.length / LINE_STEP) + 1)
    LINES.append([at(path.length * i / (samples - 1))[0] for i in range(samples)])


def straight(length):
    def build():
        run("track", Straight(length))

    return build


def slope(rise):
    def build():
        run("track", Straight(2.0, rise))

    return build


def ease(grade0, grade1):
    def build():
        run("track", Ease(4.0, grade0, grade1))

    return build


def curve(radius, degrees):
    def build():
        run("track", Curve(radius, degrees))

    return build


# The branch of a switch is a 16 m curve of 22.5 degrees, the main line runs
# straight on to the same length.
def switch(mirror):
    def build():
        branch = Curve(16, 22.5)
        run("main", Straight(branch.at(branch.length)[0][1]))
        run("branch", branch, mirror=mirror)

    return build


class Across(Straight):
    """A straight across the middle of a 4 m straight, from -x to +x."""

    def at(self, s):
        return (s - 2.0, 2.0, 0.0), math.pi / 2, 0.0


def crossing():
    run("along", Straight(4.0))
    run("across", Across(4.0))


PIECES = {
    "track_straight": straight(2.0),
    "track_straight_1": straight(1.0),
    "track_straight_4": straight(4.0),
    "track_curve_r8_22": curve(8, 22.5),
    "track_curve_r8_45": curve(8, 45),
    "track_curve_r8_90": curve(8, 90),
    "track_curve_r16_22": curve(16, 22.5),
    "track_curve_r16_45": curve(16, 45),
    "track_curve_r32_22": curve(32, 22.5),
    "track_slope_gentle": slope(0.5),
    "track_slope_steep": slope(1.0),
    "track_slope_bottom_gentle": ease(0.0, 0.25),
    "track_slope_top_gentle": ease(0.25, 0.0),
    "track_slope_bottom_steep": ease(0.0, 0.5),
    "track_slope_top_steep": ease(0.5, 0.0),
    "track_switch_right": switch(False),
    "track_switch_left": switch(True),
    "track_crossing": crossing,
}
