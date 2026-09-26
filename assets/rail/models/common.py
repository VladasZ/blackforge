import math
import os

import bmesh
import bpy
from mathutils import Matrix, Vector

# Each slot takes a vanilla material at runtime. Wood tiles freely, stone and
# iron are atlases, so their faces are packed into the region that holds the
# plain surface. The scale is texture units per meter, measured on wood_beam,
# stone_wall_1x1 and woodiron_beam.
SLOTS = {
    "wood": {"texture": "wood.png", "scale": 0.45, "region": None},
    "stone": {"texture": "stone.png", "scale": 0.18, "region": (0.0, 0.0, 0.72, 0.4)},
    "iron": {"texture": "iron.png", "scale": 0.21, "region": (0.3, 0.76, 1.0, 1.0)},
}

TEXTURES = os.environ.get(
    "BLACKFORGE_TEXTURES",
    os.path.expanduser("~/Library/Caches/blackforge-rail/textures"),
)

AXES = {"x": 0, "y": 1, "z": 2}


# The track ends of the model being built, where other track snaps on.
SNAPS = []
# The centerlines of its tracks at the height of the sleeper bottoms, where
# trains run. A switch or a crossing has two.
LINES = []


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    SNAPS.clear()
    LINES.clear()


def snap(point):
    SNAPS.append(tuple(point))


def material(slot):
    name = "slot_" + slot
    mat = bpy.data.materials.get(name)
    if mat:
        return mat
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    bsdf = nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = 0.9
    path = os.path.join(TEXTURES, SLOTS[slot]["texture"])
    if os.path.exists(path):
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = bpy.data.images.load(path, check_existing=True)
        tex.interpolation = "Closest"
        mat.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    else:
        bsdf.inputs["Base Color"].default_value = {
            "wood": (0.3, 0.2, 0.1, 1),
            "stone": (0.5, 0.5, 0.48, 1),
            "iron": (0.15, 0.15, 0.16, 1),
        }[slot]
    if slot == "iron":
        bsdf.inputs["Metallic"].default_value = 0.6
    return mat


def _uv(bm, slot, grain):
    spec = SLOTS[slot]
    layer = bm.loops.layers.uv.verify()
    g = AXES[grain]
    for face in bm.faces:
        n = face.normal
        drop = max(range(3), key=lambda i: abs(n[i]))
        keep = [i for i in range(3) if i != drop]
        # The grain axis goes along u whenever it lies in the face plane.
        if g in keep:
            keep = [g] + [i for i in keep if i != g]
        pts = [(l.vert.co[keep[0]], l.vert.co[keep[1]]) for l in face.loops]
        s = spec["scale"]
        region = spec["region"]
        if region is None:
            for l, (a, b) in zip(face.loops, pts):
                l[layer].uv = (a * s, b * s)
            continue
        u0, v0, u1, v1 = region
        amin = min(p[0] for p in pts)
        bmin = min(p[1] for p in pts)
        span = max(max(p[0] for p in pts) - amin, max(p[1] for p in pts) - bmin, 1e-6)
        fit = min(s, (u1 - u0) / span, (v1 - v0) / span)
        for l, (a, b) in zip(face.loops, pts):
            l[layer].uv = (u0 + (a - amin) * fit, v0 + (b - bmin) * fit)


def _object(name, bm, slot, grain, loc, rot, extent):
    # UVs come from world axes, so they are laid out after the rotation.
    m = Matrix.Translation(Vector(loc))
    if rot:
        m = m @ rot
    # Every part is also a box collider, the size in its own axes and the
    # axes in the world. The game places a piece by its convex colliders.
    axes = (rot or Matrix.Identity(4)).to_3x3()
    collider = list(loc) + list(extent) + list(axes.col[1]) + list(axes.col[2])
    bm.transform(m)
    bm.normal_update()
    _uv(bm, slot, grain)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.append(material(slot))
    obj = bpy.data.objects.new(name, mesh)
    obj["slot"] = slot
    obj["collider"] = collider
    bpy.context.scene.collection.objects.link(obj)
    return obj


def box(name, size, loc, slot, grain="x", rot=None):
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, vec=Vector(size), verts=bm.verts)
    return _object(name, bm, slot, grain, loc, rot, size)


def cylinder(name, radius, depth, loc, slot, axis="x", segments=12, grain=None, radius2=None):
    bm = bmesh.new()
    bmesh.ops.create_cone(
        bm,
        cap_ends=True,
        segments=segments,
        radius1=radius,
        radius2=radius if radius2 is None else radius2,
        depth=depth,
    )
    rot = {
        "x": Matrix.Rotation(math.radians(90), 4, "Y"),
        "y": Matrix.Rotation(math.radians(90), 4, "X"),
        "z": None,
    }[axis]
    wide = 2 * max(radius, radius if radius2 is None else radius2)
    return _object(name, bm, slot, grain or axis, loc, rot, (wide, wide, depth))


def rot_z(degrees):
    return Matrix.Rotation(math.radians(degrees), 4, "Z")


def rot_x(degrees):
    return Matrix.Rotation(math.radians(degrees), 4, "X")


def rot_y(degrees):
    return Matrix.Rotation(math.radians(degrees), 4, "Y")
