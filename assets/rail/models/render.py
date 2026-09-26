"""Render preview images of one model.

blender -b --python render.py -- <model> <out dir>

A wagon or locomotive stands on track next to a vanilla cart for scale. The
cart comes from the local AssetRipper export in BLACKFORGE_EXPORT.
"""

import importlib
import math
import os
import sys

import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from common import reset  # noqa: E402
from parts import PIECE, track  # noqa: E402

EXPORT = os.environ.get(
    "BLACKFORGE_EXPORT",
    os.path.expanduser("~/Library/Caches/blackforge-rail/export/Assets"),
)

VIEWS = {
    "front": (35, 22),
    "back": (215, 22),
    "side": (0, 8),
    "top": (320, 55),
}


def scene_bounds():
    pts = [
        o.matrix_world @ Vector(c)
        for o in bpy.context.scene.objects
        if o.type == "MESH"
        for c in o.bound_box
    ]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    return lo, hi


def add_cart():
    path = os.path.join(EXPORT, "GameElements/Cart/Cart.glb")
    if not os.path.exists(path):
        print("no vanilla cart at", path)
        return
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    roots = [o for o in bpy.context.scene.objects if o not in before and o.parent is None]
    for o in roots:
        o.location.x += 3.6


def setup_world():
    scene = bpy.context.scene
    for engine in ("BLENDER_EEVEE_NEXT", "BLENDER_EEVEE"):
        try:
            scene.render.engine = engine
            break
        except TypeError:
            continue
    scene.render.resolution_x = 960
    scene.render.resolution_y = 680
    scene.view_settings.view_transform = "Standard"
    world = bpy.data.worlds.new("world")
    world.color = (0.45, 0.5, 0.55)
    scene.world = world
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.45, 0.5, 0.55, 1)
    world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.7
    sun = bpy.data.lights.new("sun", "SUN")
    sun.energy = 3.5
    sun_obj = bpy.data.objects.new("sun", sun)
    sun_obj.rotation_euler = (math.radians(50), 0, math.radians(35))
    scene.collection.objects.link(sun_obj)
    ground = bpy.data.meshes.new("ground")
    ground.from_pydata([(-40, -40, 0), (40, -40, 0), (40, 40, 0), (-40, 40, 0)], [], [(0, 1, 2, 3)])
    mat = bpy.data.materials.new("ground")
    mat.use_nodes = True
    mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.22, 0.28, 0.16, 1)
    ground.materials.append(mat)
    scene.collection.objects.link(bpy.data.objects.new("ground", ground))


def render(out_dir, name, lo, hi):
    scene = bpy.context.scene
    center = (lo + hi) / 2
    radius = (hi - lo).length / 2
    cam = bpy.data.cameras.new("cam")
    cam.lens = 40
    cam_obj = bpy.data.objects.new("cam", cam)
    scene.collection.objects.link(cam_obj)
    scene.camera = cam_obj
    for view, (azimuth, elevation) in VIEWS.items():
        a = math.radians(azimuth)
        e = math.radians(elevation)
        dist = radius * 2.6
        cam_obj.location = center + Vector((math.cos(a) * math.cos(e), math.sin(a) * math.cos(e), math.sin(e))) * dist
        cam_obj.rotation_euler = (center - cam_obj.location).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(out_dir, f"{name}-{view}.png")
        bpy.ops.render.render(write_still=True)


def main():
    args = sys.argv[sys.argv.index("--") + 1 :]
    name, out_dir = args[0], os.path.abspath(args[1])
    os.makedirs(out_dir, exist_ok=True)
    reset()
    importlib.import_module(name).build()
    lo, hi = scene_bounds()
    if name in ("wagon", "locomotive"):
        track("under", y0=-3 * PIECE / 2, length=3 * PIECE)
        add_cart()
    elif name == "station":
        track("beside", y0=-2 * PIECE, length=4 * PIECE)
    lo, hi = scene_bounds()
    setup_world()
    render(out_dir, name, lo, hi)


main()
