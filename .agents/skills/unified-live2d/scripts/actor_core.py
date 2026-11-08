"""Offline reference math for a unified actor. No renderer or app integration."""
import argparse
import copy
import math

EPS = 1e-9
EASES = {"linear", "smoothstep", "smootherstep", "smootherstep_squared",
         "smootherstep_complement_squared"}


def number(value, label):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise ValueError(f"{label}: expected a finite number")
    return float(value)


def normalize(model):
    """Validate and expand phase shorthand. Return a copy; never evaluate formulas."""
    m = copy.deepcopy(model)
    if m.get("format") not in {"unified-actor-1", "lexington-parametric-actor-draft-1"}:
        raise ValueError("unsupported actor format")
    canvas = m.get("canvas", [])
    if len(canvas) != 2 or any(number(v, "canvas") <= 0 or int(v) != v for v in canvas):
        raise ValueError("canvas: expected two positive integers")
    pivot = m.get("root_pivot", [])
    if len(pivot) != 2:
        raise ValueError("root_pivot: expected [x,y]")
    for v in pivot:
        number(v, "root_pivot")
    channels = m.get("channels")
    if not isinstance(channels, dict) or not channels:
        raise ValueError("channels: expected a nonempty object")
    for name, c in channels.items():
        bounds = c.get("range", [])
        if len(bounds) != 2:
            raise ValueError(f"{name}: expected range [min,max]")
        lo, hi = (number(v, name) for v in bounds)
        d = number(c.get("default"), name)
        if not lo <= d <= hi:
            raise ValueError(f"{name}: default outside range")
    for name, a in m.get("actions", {}).items():
        duration = number(a.get("duration"), f"{name}.duration")
        if duration <= 0:
            raise ValueError(f"{name}: duration must be positive")
        if "tracks" not in a:
            channel = a.get("channel")
            if channel not in channels:
                raise ValueError(f"{name}: unknown channel")
            d = channels[channel]["default"]
            if "close" in a:
                p, h, q = (number(a.get(k), name) for k in ("close", "hold", "open"))
                peak, up, down = a.get("closed_value", 0), "smoothstep", "smoothstep"
            else:
                p, h, q = (number(a.get(k), name) for k in ("rise", "hold", "fall"))
                peak = a.get("amplitude")
                up, down = "smootherstep_squared", "smootherstep_complement_squared"
            if min(p, q) <= 0 or h < 0 or abs(p + h + q - duration) > EPS:
                raise ValueError(f"{name}: phase durations must sum to duration")
            if a.get("start", d) != d or a.get("end", d) != d:
                raise ValueError(f"{name}: endpoints must equal channel default")
            keys = [{"t": 0, "v": d, "ease": up}, {"t": p, "v": peak, "ease": down}]
            if h > 0:
                keys[1]["ease"] = "linear"
                keys.append({"t": p + h, "v": peak, "ease": down})
            keys.append({"t": duration, "v": d})
            a["tracks"] = {channel: keys}
        if not isinstance(a["tracks"], dict) or not a["tracks"]:
            raise ValueError(f"{name}: tracks must be nonempty")
        for channel, keys in a["tracks"].items():
            if channel not in channels or not isinstance(keys, list) or len(keys) < 2:
                raise ValueError(f"{name}: unknown channel or invalid keys: {channel}")
            prev = -math.inf
            lo, hi = channels[channel]["range"]
            for key in keys:
                t, v = number(key.get("t"), name), number(key.get("v"), name)
                if t <= prev or not 0 <= t <= duration or not lo <= v <= hi:
                    raise ValueError(f"{name}/{channel}: invalid key time or value")
                if key.get("ease", "linear") not in EASES:
                    raise ValueError(f"{name}: unsupported easing")
                prev = t
            if abs(keys[0]["t"]) > EPS or abs(keys[-1]["t"] - duration) > EPS:
                raise ValueError(f"{name}: tracks must span the entire action")
            if any(abs(k["v"] - channels[channel]["default"]) > EPS for k in (keys[0], keys[-1])):
                raise ValueError(f"{name}: track must return to neutral")
    idle = m.get("shared_idle", {})
    if idle.get("waveform", "sine_cubed") not in {"sine", "sine_cubed"}:
        raise ValueError("unsupported idle waveform")
    for group in ("breathing", "sway"):
        spec = idle.get(group, {})
        if number(spec.get("period", 1), f"idle.{group}.period") <= 0:
            raise ValueError("idle period must be positive")
        for field in ("max_dy_px", "rotation_clockwise_degrees_per_wave", "translation_x_px_per_wave"):
            number(spec.get(field, 0), field)
    return m


def ease(u, kind):
    u = max(0.0, min(1.0, u))
    if kind == "linear":
        return u
    if kind == "smoothstep":
        return u * u * (3 - 2 * u)
    s = u ** 3 * (10 - 15 * u + 6 * u * u)
    if kind == "smootherstep":
        return s
    if kind == "smootherstep_squared":
        return s * s
    if kind == "smootherstep_complement_squared":
        return 1 - (1 - s) ** 2
    raise ValueError(f"unsupported easing: {kind}")


def track_value(keys, t):
    if t <= keys[0]["t"]:
        return keys[0]["v"]
    for left, right in zip(keys, keys[1:]):
        if t <= right["t"]:
            w = ease((t - left["t"]) / (right["t"] - left["t"]), left.get("ease", "linear"))
            return left["v"] + (right["v"] - left["v"]) * w
    return keys[-1]["v"]


def idle_sample(model, t):
    number(t, "global_time")
    idle = model.get("shared_idle", {})
    def wave(spec):
        w = math.sin(2 * math.pi * t / spec.get("period", 1))
        if idle.get("waveform", "sine_cubed") == "sine_cubed":
            w = w ** 3
        return 0.0 if abs(w) < 1e-4 else w
    breath, sway = idle.get("breathing", {}), idle.get("sway", {})
    bw, sw = wave(breath), wave(sway)
    return {"breath_wave": bw, "breath_dy": bw * breath.get("max_dy_px", 0),
            "rotation_degrees": sw * sway.get("rotation_clockwise_degrees_per_wave", 0),
            "translation_x": sw * sway.get("translation_x_px_per_wave", 0)}


def sample(model, events, global_time):
    """Use a normalized model. Events are {action, start}; time is global seconds."""
    params = {k: v["default"] for k, v in model["channels"].items()}
    owned = set()
    for event in events:
        a = model["actions"][event["action"]]
        local = global_time - number(event["start"], "event.start")
        if local < -EPS or local >= a["duration"] - EPS:
            continue
        conflict = owned.intersection(a["tracks"])
        if conflict:
            raise ValueError(f"overlapping channel ownership: {sorted(conflict)}")
        owned.update(a["tracks"])
        for channel, keys in a["tracks"].items():
            params[channel] = track_value(keys, max(0, local))
    return {"parameters": params, "idle": idle_sample(model, global_time)}


def queue_event(model, events, action, requested_time):
    """Append a trigger, permitting at most one pending event per channel. None=full."""
    number(requested_time, "requested_time")
    channels = set(model["actions"][action]["tracks"])
    start = requested_time
    for event in events:
        other = model["actions"][event["action"]]
        if not channels.intersection(other["tracks"]):
            continue
        end = event["start"] + other["duration"]
        if end <= requested_time + EPS:
            continue
        if event["start"] > requested_time + EPS:
            return None
        start = max(start, end)
    event = {"action": action, "start": start}
    events.append(event)
    return event


def solve_ik(shoulder, elbow, wrist, target, bend):
    """Two bones, scale=1; bend is a persistent +/-1 in image y-down coordinates."""
    if bend not in (-1, 1):
        raise ValueError("bend must be a fixed +1 or -1")
    for p in (shoulder, elbow, wrist, target):
        if len(p) != 2:
            raise ValueError("IK points must be [x,y]")
        for v in p:
            number(v, "IK coordinate")
    a, b = math.dist(shoulder, elbow), math.dist(elbow, wrist)
    d = math.dist(shoulder, target)
    if min(a, b, d) <= EPS or d > a + b + EPS or d < abs(a - b) - EPS:
        raise ValueError("unreachable or degenerate IK; adjust target, do not scale arms")
    x = (a*a - b*b + d*d) / (2*d)
    h = math.sqrt(max(0, a*a - x*x)) * bend
    ux, uy = (target[0] - shoulder[0]) / d, (target[1] - shoulder[1]) / d
    return (shoulder[0] + ux*x - uy*h, shoulder[1] + uy*x + ux*h)


def transform(point, matrix):
    """Affine [a,b,c,d,tx,ty]: (a*x+c*y+tx, b*x+d*y+ty)."""
    a, b, c, d, tx, ty = matrix
    x, y = point
    return (a*x + c*y + tx, b*x + d*y + ty)


def inverse(matrix):
    a, b, c, d, tx, ty = (number(v, "affine") for v in matrix)
    det = a*d - b*c
    if abs(det) < EPS:
        raise ValueError("singular affine transform")
    return (d/det, -b/det, -c/det, a/det, (c*ty-d*tx)/det, (b*tx-a*ty)/det)


IDENTITY = (1, 0, 0, 1, 0, 0)


def root_matrix(model, t):
    idle = idle_sample(model, t)
    angle = math.radians(idle["rotation_degrees"])
    c, s = math.cos(angle), math.sin(angle)
    x, y = model["root_pivot"]
    return (c, s, -s, c, x-c*x+s*y+idle["translation_x"], y-s*x-c*y)


def hit_test(model, point, parameters, *, root=IDENTITY, viewport=IDENTITY, parts=None):
    """Inverse viewport -> root -> optional per-region rigid part -> translation."""
    p = transform(transform(point, inverse(viewport)), inverse(root))
    ordered = sorted(enumerate(model.get("hit_regions", [])),
                     key=lambda pair: (pair[1].get("priority", 0), pair[0]), reverse=True)
    for _, region in ordered:
        q = transform(p, inverse((parts or {}).get(region["id"], IDENTITY)))
        translation = region.get("translation", [0, 0])
        if "local_transform" in region:
            # Legacy manifest alias; never execute formula strings.
            import re
            match = re.fullmatch(r"translate\(0,-([A-Za-z_][A-Za-z0-9_]*)\)", region["local_transform"])
            if not match:
                raise ValueError("unsupported local_transform; use typed translation")
            translation = [0, {"channel": match[1], "scale": -1}]
        def offset(value):
            if isinstance(value, dict):
                return parameters[value["channel"]] * number(value.get("scale", 1), "translation.scale")
            return number(value, "translation")
        x, y = q[0]-offset(translation[0]), q[1]-offset(translation[1])
        if region["shape"] == "ellipse":
            cx, cy = region["center"]
            rx, ry = region["radii"]
            if min(rx, ry) <= 0:
                raise ValueError("ellipse radii must be positive")
            inside = ((x-cx)/rx)**2 + ((y-cy)/ry)**2 <= 1
        elif region["shape"] == "polygon":
            vertices = region["points"]
            if len(vertices) < 3:
                raise ValueError("polygon requires at least three vertices")
            inside = False
            for (ax, ay), (bx, by) in zip(vertices, vertices[1:] + vertices[:1]):
                cross = (x-ax)*(by-ay)-(y-ay)*(bx-ax)
                if abs(cross) < EPS and min(ax,bx)-EPS <= x <= max(ax,bx)+EPS and min(ay,by)-EPS <= y <= max(ay,by)+EPS:
                    inside = True
                    break
                if (ay > y) != (by > y) and x < (bx-ax)*(y-ay)/(by-ay)+ax:
                    inside = not inside
        else:
            raise ValueError("unsupported hit region shape")
        if inside:
            return region
    return None


def self_test():
    model = normalize({"format":"unified-actor-1", "canvas":[100,100], "root_pivot":[50,90],
        "shared_idle":{"sway":{"period":6,"rotation_clockwise_degrees_per_wave":1}},
        "channels":{"a":{"default":0,"range":[0,10]},"b":{"default":0,"range":[0,1]}},
        "actions":{"lift":{"duration":2,"tracks":{"a":[{"t":0,"v":0},{"t":1,"v":10},{"t":2,"v":0}]}},
                   "eye":{"duration":1,"tracks":{"b":[{"t":0,"v":0},{"t":0.5,"v":1},{"t":1,"v":0}]}}},
        "hit_regions":[{"id":"prop","shape":"polygon","points":[[0,0],[10,0],[10,10],[0,10]],
                        "translation":[0,{"channel":"a","scale":-1}],"action":"lift"}]})
    events = []
    assert queue_event(model, events, "lift", 0.37)["start"] == 0.37
    assert queue_event(model, events, "lift", 0.5)["start"] == 2.37
    assert queue_event(model, events, "lift", 0.6) is None
    assert queue_event(model, events, "eye", 0.5)["start"] == 0.5
    for t in (0.37, 2.37, 4.37):
        assert abs(sample(model, events, t)["parameters"]["a"]) < EPS
        assert sample(model, events, t)["idle"] == sample(model, [], t)["idle"]
    assert sample(model, events, 1)["parameters"] == {"a":6.3, "b":1.0}
    try:
        sample(model, [{"action":"lift","start":0},{"action":"lift","start":0.1}], 0.5)
    except ValueError:
        pass
    else:
        raise AssertionError("conflicting ownership accepted")
    for y in range(-5, 6):
        e = solve_ik((0,0), (8,6), (16,0), (16,y/10), 1)
        assert abs(math.dist((0,0), e)-10) < EPS
        assert abs(math.dist(e, (16,y/10))-10) < EPS
        assert 16*e[1]-(y/10)*e[0] > 0
    for t in (0, 1.3, 4):
        root = root_matrix(model, t)
        view = (2,0,0,2,40,90)
        screen = transform(transform((5,-5), root), view)
        assert hit_test(model, screen, {"a":10}, root=root, viewport=view)["id"] == "prop"
        assert hit_test(model, screen, {"a":0}, root=root, viewport=view) is None
    print("PASS: clocks, endpoints, overlap, bounded queue, fixed-pole IK, moving hit regions")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
    else:
        parser.print_help()
