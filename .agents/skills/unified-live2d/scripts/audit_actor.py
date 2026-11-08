"""Audit portable actor data and optionally decoded looping GIF previews.

Numeric checks do not prove visual quality or actual runtime integration.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys

from actor_core import normalize, number, sample, hit_test


def package_path(root, value):
    if not isinstance(value, str) or not value or "\\" in value or ":" in value:
        raise ValueError(f"asset reference must be a portable relative path: {value!r}")
    path = Path(value)
    if path.is_absolute() or ".." in path.parts:
        raise ValueError(f"asset path escapes package: {value!r}")
    resolved = (root / path).resolve()
    if not resolved.is_relative_to(root) or not resolved.is_file():
        raise ValueError(f"asset missing or escapes package: {value!r}")
    return resolved


def asset_refs(value, key="assets"):
    """The compact contract stores paths in these fields, not arbitrary text."""
    path_keys = {"neutral", "path", "coverage", "background", "source_rgba", "arm_layers",
                 "bouquet", "bodice_replace_mask", "shoulder_replace_mask", "body_breath_mesh"}
    if isinstance(value, dict):
        for k, v in value.items():
            yield from asset_refs(v, k)
    elif isinstance(value, list):
        for item in value:
            yield from asset_refs(item, key)
    elif isinstance(value, str) and key in path_keys:
        yield value


def check_mesh(path):
    mesh = json.loads(path.read_text(encoding="utf-8"))
    if mesh.get("vertex_format") != ["x", "y", "breath_dx", "breath_dy"]:
        raise ValueError("breath mesh must declare [x,y,breath_dx,breath_dy]")
    vertices, indices = mesh.get("vertices", []), mesh.get("indices", [])
    if not vertices or not indices or len(indices) % 3:
        raise ValueError("breath mesh must have vertices and triangle indices")
    for row in vertices:
        if len(row) != 4:
            raise ValueError("mesh vertex stride must be four")
        for v in row:
            number(v, "mesh vertex")
    if any(isinstance(i, bool) or not isinstance(i, int) or not 0 <= i < len(vertices) for i in indices):
        raise ValueError("mesh triangle index outside vertex array")
    for j in range(0, len(indices), 3):
        tri = [vertices[i] for i in indices[j:j+3]]
        def area(wave):
            a, b, c = [(v[0]+wave*v[2], v[1]+wave*v[3]) for v in tri]
            return (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])
        base = area(0)
        if abs(base) < 1e-9 or any(area(w)*base <= 0 for w in (-1, 1)):
            raise ValueError(f"folded/degenerate breath triangle {j//3}")
        # Area is quadratic in wave; also test its interior extremum.
        am, ap = area(-1), area(1)
        qa, qb = (ap+am)/2-base, (ap-am)/2
        if abs(qa) > 1e-12:
            w = -qb/(2*qa)
            if -1 < w < 1 and area(w)*base <= 0:
                raise ValueError(f"breath triangle folds between extrema: {j//3}")
    return {"vertices":len(vertices), "triangles":len(indices)//3}


def gif_palettes(path):
    """Read actual global/local color tables, independently of Pillow's RGB conversion."""
    data, tables = path.read_bytes(), []
    if data[:6] not in (b"GIF87a", b"GIF89a"):
        raise ValueError("preview is not a GIF")
    packed, pos = data[10], 13
    def table(packed, pos):
        size = 3 * (2 ** ((packed & 7)+1))
        return data[pos:pos+size], pos+size
    global_table = None
    if packed & 128:
        global_table, pos = table(packed, pos)
    def skip_blocks(pos):
        while data[pos]:
            pos += 1 + data[pos]
        return pos+1
    while pos < len(data):
        tag, pos = data[pos], pos+1
        if tag == 0x3B:
            break
        if tag == 0x21:
            pos = skip_blocks(pos+1)
        elif tag == 0x2C:
            packed, pos = data[pos+8], pos+9
            active = global_table
            if packed & 128:
                active, pos = table(packed, pos)
            if active is None:
                raise ValueError("GIF image without a palette")
            tables.append(active)
            pos = skip_blocks(pos+1)  # LZW code size, then data subblocks
        else:
            raise ValueError(f"unexpected GIF block: {tag}")
    if not tables:
        raise ValueError("GIF contains no images")
    return tables


def pixel_hash(image):
    rgba = bytearray(image.convert("RGBA").tobytes())
    for i in range(0, len(rgba), 4):
        if rgba[i+3] == 0:
            rgba[i:i+3] = b"\0\0\0"
    return hashlib.sha256(rgba).hexdigest()


def audit(model_path, clips=(), require_palette=False, require_endpoints=False):
    model_path = Path(model_path).resolve()
    root = model_path.parent
    model = normalize(json.loads(model_path.read_text(encoding="utf-8-sig")))
    refs = set(asset_refs(model.get("assets", {})))
    refs.add(model.get("neutral"))
    paths = {ref:package_path(root, ref) for ref in refs}
    result = {"format":model["format"], "assets_checked":len(paths),
              "actions_checked":list(model.get("actions", {})), "runtime_integration_checked":False}
    for name, action in model.get("actions", {}).items():
        start = 0.713  # Deliberately away from neutral idle phase.
        for t in (start, start+action["duration"]):
            triggered = sample(model, [{"action":name,"start":start}], t)
            idle = sample(model, [], t)
            if triggered != idle:
                raise ValueError(f"{name}: endpoint differs from idle at same global time")
    for region in model.get("hit_regions", []):
        if region.get("action") not in model.get("actions", {}):
            raise ValueError(f"hotspot {region.get('id')}: unknown action")
        # Exercise shape and typed translation validation, including non-finite input.
        for key in ("center", "radii"):
            if key in region:
                if len(region[key]) != 2:
                    raise ValueError(f"{region.get('id')}: {key} requires two numbers")
                for v in region[key]:
                    number(v, key)
        for p in region.get("points", []):
            if len(p) != 2:
                raise ValueError("hotspot point requires two numbers")
            for v in p:
                number(v, "hotspot point")
        hit_test({"hit_regions":[region]}, (0,0), {k:c["default"] for k,c in model["channels"].items()})
    mesh_ref = model.get("assets", {}).get("body_breath_mesh")
    if mesh_ref:
        result["breath_mesh"] = check_mesh(package_path(root, mesh_ref))
    pngs = [p for p in paths.values() if p.suffix.lower() == ".png"]
    if pngs or clips:
        from PIL import Image
        sizes = {}
        for path in pngs:
            with Image.open(path) as image:
                image.load()
                sizes[path] = image.size
        if sizes.get(package_path(root, model["neutral"])) != tuple(model["canvas"]):
            raise ValueError("neutral PNG dimensions differ from canvas")
        assets = model.get("assets", {})
        for key in ("background", "source_rgba", "arm_layers", "bouquet", "bodice_replace_mask", "shoulder_replace_mask"):
            # These legacy roles specifically declare full-canvas layers. Generic
            # {path,offset/rect/uv} assets may be cropped and are not forced full-size.
            values = assets.get(key, [])
            values = values if isinstance(values, list) else [values]
            for ref in values:
                if isinstance(ref, str) and sizes.get(package_path(root, ref)) != tuple(model["canvas"]):
                    raise ValueError(f"{key}: expected full-canvas PNG")
        atlas = assets.get("eye_atlas")
        if atlas:
            cw, ch = atlas["cell"]
            cols, states = atlas["columns"], atlas["states"]
            if any(isinstance(v, bool) or not isinstance(v, int) or v <= 0 for v in (cw,ch,cols,states)):
                raise ValueError("atlas cell/columns/states must be positive integers")
            w, h = sizes[package_path(root, atlas["path"])]
            if w != cw*cols or h % ch or states > cols*(h//ch):
                raise ValueError("atlas geometry cannot hold declared states")
            if "coverage" in atlas and sizes[package_path(root, atlas["coverage"])] != (cw,ch):
                raise ValueError("eye coverage dimensions differ from atlas cell")
            if "face_box" in atlas:
                x0,y0,x1,y1 = atlas["face_box"]
                if (x1-x0,y1-y0) != (cw,ch) or not (0 <= x0 < x1 <= model["canvas"][0] and 0 <= y0 < y1 <= model["canvas"][1]):
                    raise ValueError("atlas face_box inconsistent with canvas/cell")
        result["pngs_checked"] = len(pngs)
        all_palettes, first_frames, last_frames, preview_reports = [], [], [], []
        for clip in clips:
            path = Path(clip)
            tables = gif_palettes(path)
            all_palettes.extend(tables)
            with Image.open(path) as image:
                if image.size != tuple(model["canvas"]):
                    raise ValueError(f"{path.name}: dimensions differ from canvas")
                first = pixel_hash(image)
                duration = 0
                for i in range(image.n_frames):
                    image.seek(i)
                    duration += image.info.get("duration", 0)
                last = pixel_hash(image)
                first_frames.append(first)
                last_frames.append(last)
                preview_reports.append({"file":str(path),"decoded_frames":image.n_frames,
                                        "duration_ms":duration,"first_equals_last":first==last})
        palette_match = len(set(all_palettes)) <= 1
        endpoint_match = len(set(first_frames+last_frames)) <= 1
        if require_palette and (not clips or not palette_match):
            raise ValueError("GIF palettes are not identical across all image frames/previews")
        if require_endpoints and (not clips or not endpoint_match):
            raise ValueError("decoded preview first/last frames are not all identical")
        result.update(previews=preview_reports, shared_palette=palette_match,
                      all_preview_endpoints_identical=endpoint_match)
    elif require_palette or require_endpoints:
        raise ValueError("preview checks require --clips")
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--model", required=True)
    p.add_argument("--clips", nargs="*", default=[])
    p.add_argument("--require-shared-palette", action="store_true")
    p.add_argument("--require-matching-endpoints", action="store_true")
    p.add_argument("--report")
    args = p.parse_args()
    try:
        report = audit(args.model, args.clips, args.require_shared_palette, args.require_matching_endpoints)
    except (ValueError, KeyError, TypeError, IndexError, OSError, ImportError) as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 1
    content = json.dumps(report, ensure_ascii=False, indent=2)
    if args.report:
        Path(args.report).write_text(content+"\n", encoding="utf-8")
    print(content)
    return 0


if __name__ == "__main__":
    sys.exit(main())
