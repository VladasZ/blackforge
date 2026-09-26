"""Write every model as a mesh file and a hammer icon for the plugin.

blender -b --python export.py

The mesh file has one part per material slot, then one box collider per
model part: the center, the size, and the forward and up axes. Last come
the track ends, where other track snaps on, and the track centerlines. Blender is right handed with z
up, Unity left handed with y up. Swapping y and z converts both at once. The mirror
keeps the winding as seen from outside, but Unity culls the other way round
from Blender, so every triangle is written in reverse order.
"""

import importlib
import math
import os
import struct
import sys

import bmesh
import bpy
from mathutils import Vector

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from common import LINES, SLOTS, SNAPS, reset  # noqa: E402
from tracks import PIECES  # noqa: E402

MODELS = {name: build for name, build in PIECES.items()}
for _name in ("station", "locomotive", "wagon"):
    MODELS[_name] = importlib.import_module(_name).build
MESHES = os.path.join(HERE, "..", "meshes")
ICONS = os.path.join(HERE, "..", "icons")
MAGIC = b"BFM4"


def collect():
    parts = {slot: ([], []) for slot in SLOTS}
    colliders = []
    for obj in bpy.context.scene.objects:
        if obj.type != "MESH":
            continue
        bm = bmesh.new()
        bm.from_mesh(obj.data)
        bm.transform(obj.matrix_world)
        bmesh.ops.triangulate(bm, faces=bm.faces)
        bm.normal_update()
        uv = bm.loops.layers.uv.active
        verts, tris = parts[obj["slot"]]
        c = list(obj["collider"])
        # Unity local y is Blender local z, so the size and the axes swap too.
        colliders.append((c[0], c[2], c[1], c[3], c[5], c[4], c[6], c[8], c[7], c[9], c[11], c[10]))
        for face in bm.faces:
            base = len(verts)
            for loop in face.loops:
                p = loop.vert.co
                n = face.normal
                t = loop[uv].uv
                verts.append((p.x, p.z, p.y, n.x, n.z, n.y, t.x, t.y))
            tris.extend((base, base + 2, base + 1))
        bm.free()
    return {slot: part for slot, part in parts.items() if part[1]}, colliders


def write_mesh(name, parts, colliders):
    path = os.path.join(MESHES, f"{name}.bfm")
    with open(path, "wb") as out:
        out.write(MAGIC)
        out.write(struct.pack("<i", len(parts)))
        for slot, (verts, tris) in parts.items():
            encoded = slot.encode()
            out.write(struct.pack("<i", len(encoded)))
            out.write(encoded)
            out.write(struct.pack("<i", len(verts)))
            for v in verts:
                out.write(struct.pack("<8f", *v))
            out.write(struct.pack("<i", len(tris)))
            out.write(struct.pack(f"<{len(tris)}i", *tris))
        out.write(struct.pack("<i", len(colliders)))
        for c in colliders:
            out.write(struct.pack("<12f", *c))
        out.write(struct.pack("<i", len(SNAPS)))
        for x, y, z in SNAPS:
            out.write(struct.pack("<3f", x, z, y))
        out.write(struct.pack("<i", len(LINES)))
        for line in LINES:
            out.write(struct.pack("<i", len(line)))
            for x, y, z in line:
                out.write(struct.pack("<3f", x, z, y))
    print("wrote", path, {slot: len(t) // 3 for slot, (_, t) in parts.items()}, "triangles")


def write_icon(name):
    scene = bpy.context.scene
    for engine in ("BLENDER_EEVEE_NEXT", "BLENDER_EEVEE"):
        try:
            scene.render.engine = engine
            break
        except TypeError:
            continue
    scene.render.resolution_x = 128
    scene.render.resolution_y = 128
    scene.render.film_transparent = True
    scene.view_settings.view_transform = "Standard"
    world = bpy.data.worlds.new("world")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.8
    scene.world = world
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 3.5
    sun.rotation_euler = (math.radians(50), 0, math.radians(35))
    scene.collection.objects.link(sun)

    pts = [o.matrix_world @ Vector(c) for o in scene.objects if o.type == "MESH" for c in o.bound_box]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    center = (lo + hi) / 2
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = (hi - lo).length * 0.95
    a, e = math.radians(35), math.radians(28)
    cam.location = center + Vector((math.cos(a) * math.cos(e), math.sin(a) * math.cos(e), math.sin(e))) * 30
    cam.rotation_euler = (center - cam.location).to_track_quat("-Z", "Y").to_euler()
    scene.collection.objects.link(cam)
    scene.camera = cam
    scene.render.filepath = os.path.join(ICONS, f"{name}.png")
    bpy.ops.render.render(write_still=True)


def main():
    os.makedirs(MESHES, exist_ok=True)
    os.makedirs(ICONS, exist_ok=True)
    for name, build in MODELS.items():
        reset()
        build()
        write_mesh(name, *collect())
        write_icon(name)


main()
