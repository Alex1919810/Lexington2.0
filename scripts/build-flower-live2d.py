"""Register AI-completed Flower layers and build the native actor (offline only).

Requires NumPy, OpenCV and Pillow plus the ignored generated source directory.
Default output is a draft; --publish installs the reviewed model, actor.json last.
AI performs painting; this script only registers, assigns ownership and packs it.
"""
import argparse
import json
from pathlib import Path
import shutil

import cv2
import numpy as np
from PIL import Image, ImageDraw

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--publish', action='store_true')
args = parser.parse_args()
ROOT = Path(__file__).resolve().parents[1]
DRAFT = ROOT / 'resource/illustration/lexington/live2d-drafts/Flower_Normal'
MODEL = DRAFT / 'model'
LIVE = ROOT / 'resource/illustration/lexington/live2d/Flower_Normal'
names = ['body', 'hair', 'left-arm', 'right-arm', 'accessories']
missing = [name for name in names if not (DRAFT / 'generated' / (name + '.png')).is_file()]
if missing:
    parser.error('缺少本地 AI 制作源：' + ', '.join(missing) + '；正式运行不需要重建素材。')
MODEL.mkdir(parents=True, exist_ok=True)


def rgba(path, size=1024):
    a = np.array(Image.open(path).convert('RGBA')).astype(np.float32) / 255
    a[:, :, :3] *= a[:, :, 3:4]
    a = cv2.resize(a, (size, size), interpolation=cv2.INTER_AREA)
    return a


def polygon(points):
    im = Image.new('L', (1024, 1024))
    ImageDraw.Draw(im).polygon(points, fill=255)
    return np.array(im).astype(np.float32) / 255


def rect(x0, y0, x1, y1):
    return polygon([(x0, y0), (x1, y0), (x1, y1), (x0, y1)])


def over(bottom, top):
    return top + bottom * (1 - top[:, :, 3:4])


def register(image, source, target):
    # Smooth inverse TPS registration in premultiplied alpha. Landmark coordinates
    # are measured on 1024px normalized generator output, not requested dimensions.
    controls = np.float64(target)
    values = np.float64(source)
    delta = controls[:, None] - controls[None, :]
    r2 = (delta * delta).sum(2)
    p = np.c_[np.ones(len(controls)), controls]
    system = np.block([[r2 * np.log(r2 + 1e-12) + np.eye(len(controls)) * .01, p],
                       [p.T, np.zeros((3, 3))]])
    coeff = np.linalg.solve(system, np.r_[values, np.zeros((3, 2))])
    yy, xx = np.mgrid[:1024, :1024]
    points = np.c_[xx.ravel(), yy.ravel()]
    delta = points[:, None] - controls[None, :]
    r2 = (delta * delta).sum(2)
    uv = (r2 * np.log(r2 + 1e-12)) @ coeff[:len(controls)] + np.c_[np.ones(len(points)), points] @ coeff[len(controls):]
    return cv2.remap(image, uv[:, 0].reshape(1024, 1024).astype(np.float32),
                     uv[:, 1].reshape(1024, 1024).astype(np.float32), cv2.INTER_LINEAR,
                     borderMode=cv2.BORDER_CONSTANT)


def save(name, pixels):
    pixels = cv2.resize(pixels, (512, 512), interpolation=cv2.INTER_AREA)
    alpha = pixels[:, :, 3:4]
    straight = np.divide(pixels[:, :, :3], alpha, out=np.zeros_like(pixels[:, :, :3]), where=alpha > 1e-7)
    result = np.uint8(np.clip(np.concatenate([straight, alpha], 2) * 255 + .5, 0, 255))
    result[result[:, :, 3] == 0] = 0
    Image.fromarray(result).save(MODEL / name)


source = rgba(ROOT / 'resource/illustration/lexington/png/Flower_Normal.png')
generated = {}
for name in names:
    a = rgba(DRAFT / 'generated' / (name + '.png'))
    # Discard detached generator dust only; preserve the alpha of real free edges.
    count, labels, stats, _ = cv2.connectedComponentsWithStats(np.uint8(a[:, :, 3] > .12))
    keep = np.zeros((1024, 1024), np.uint8)
    for label in range(1, count):
        if stats[label, cv2.CC_STAT_AREA] >= (18 if name == 'accessories' else 100):
            keep[labels == label] = 1
    support = cv2.dilate(keep, np.ones((5, 5), np.uint8))
    a[support == 0] = 0
    generated[name] = a

body = register(generated['body'],
    [[465, 99], [511, 113], [479, 151], [472, 178], [413, 218], [551, 220],
     [483, 326], [358, 551], [685, 489], [441, 568], [547, 549], [461, 648],
     [590, 737], [445, 893], [449, 968]],
    [[475, 285], [511, 296], [490, 326], [484, 338], [437, 372], [550, 372],
     [493, 455], [398, 629], [673, 564], [467, 624], [560, 601], [499, 707],
     [580, 775], [469, 894], [475, 957]])
hair = register(generated['hair'],
    [[490, 221], [462, 291], [535, 313], [466, 372], [218, 521], [299, 584],
     [411, 630], [563, 505], [574, 388]],
    [[487, 240], [465, 281], [527, 298], [487, 330], [315, 418], [366, 466],
     [432, 490], [535, 470], [548, 370]])
left = register(generated['left-arm'],
    [[420, 410], [407, 477], [357, 548], [345, 579]],
    [[426, 421], [410, 475], [366, 545], [355, 577]])
right = register(generated['right-arm'],
    [[534, 385], [581, 432], [546, 508], [507, 531]],
    [[548, 390], [582, 432], [547, 503], [516, 529]])
atlas = generated['accessories']

# Ownership polygons follow the source painting. Visible source RGBA wins; AI
# textures supply the underpainting that becomes visible after moving a part.
# Screen-left means the bouquet arm. Trace its complete inner elbow/forearm:
# the earlier straight cut assigned a small skin triangle to the moving hair.
left_owner = polygon([(415, 412), (437, 421), (441, 437), (440, 451),
    (437, 467), (432, 478), (425, 488), (416, 498), (404, 510), (394, 522),
    (384, 535), (376, 547), (374, 566), (375, 584), (358, 595), (339, 588),
    (346, 573), (346, 557), (356, 539), (374, 509), (396, 475), (411, 445)])
right_owner = polygon([(539, 389), (563, 385), (587, 413), (605, 430),
    (599, 456), (584, 489), (565, 518), (548, 534), (532, 546),
    (509, 549), (488, 537), (486, 519), (505, 507), (534, 499),
    (547, 480), (556, 444), (552, 429), (536, 409)])
color = np.divide(source[:, :, :3], source[:, :, 3:4],
                  out=np.zeros_like(source[:, :, :3]), where=source[:, :, 3:4] > 1e-7)
elbow_skin = ((color[:, :, 0] > .89) & (color[:, :, 1] > .65)
              & (color[:, :, 0] > color[:, :, 2] + .035)) | (color.min(axis=2) > .94)
elbow_edge = cv2.dilate(np.uint8(elbow_skin), np.ones((3, 3), np.uint8))
# The adjacent pale hair strand is not part of the widened skin ownership.
elbow_band = rect(419, 450, 443, 492)
left_owner *= np.maximum(1 - elbow_band, elbow_edge)
skin = (color[:, :, 0] > color[:, :, 2] + .018) & (color[:, :, 0] > color[:, :, 1] + .007)
# Include the whole antialiased arm edge; leaving old skin in the fixed body
# creates a stationary white outline when the arm rotates away.
right_owner *= cv2.dilate(np.uint8(skin), np.ones((3, 3), np.uint8))
bracelet_owner = polygon([(535, 493), (547, 490), (563, 502), (561, 513),
    (554, 521), (541, 518), (531, 508)])
right_owner *= 1 - bracelet_owner
hair_owner = polygon([(451, 274), (447, 309), (447, 341), (433, 365), (425, 390),
    (410, 416), (405, 439), (415, 465), (432, 487), (437, 494), (424, 489),
    (410, 472), (394, 479), (384, 468), (366, 485), (355, 466), (352, 447),
    (340, 455), (333, 443), (334, 426), (312, 442), (310, 414), (344, 383),
    (375, 359), (407, 322), (430, 286)])
hair_owner *= 1 - left_owner
hat_owner = polygon([(382, 276), (416, 266), (430, 241), (443, 221), (460, 208),
    (485, 206), (511, 226), (551, 216), (575, 221), (580, 232), (571, 253),
    (541, 274), (530, 275), (522, 252), (509, 238), (491, 234), (471, 244),
    (455, 259), (439, 276), (428, 291), (410, 298), (393, 290), (382, 282)])
bouquet_owner = polygon([(235, 508), (273, 505), (311, 517), (345, 533),
    (357, 557), (374, 569), (371, 578), (353, 582), (335, 572), (316, 590),
    (274, 609), (241, 606), (236, 557)])
bouquet_owner *= 1 - left_owner
train_owner = polygon([(582, 494), (625, 465), (652, 441), (666, 393),
    (662, 308), (830, 308), (838, 414), (791, 481), (734, 513),
    (678, 523), (650, 513)])

# Hidden paint is allowed only beneath an opaque source occluder. Source-visible
# pixels form a disjoint partition, including transparent RGB; never extend a
# generated free outline beyond the approved original silhouette at neutral.
opaque = cv2.erode(np.uint8(source[:, :, 3] >= .999), np.ones((5, 5), np.uint8)).astype(np.float32)
hidden_torso = polygon([(450, 352), (523, 339), (552, 391), (543, 452),
    (573, 481), (574, 553), (491, 569), (440, 518), (426, 433)])
hidden_hair = polygon([(446, 281), (470, 301), (505, 336), (540, 388),
    (542, 461), (522, 482), (479, 477), (430, 476), (409, 443), (424, 366)])
hat = register(atlas * rect(370, 195, 590, 309)[:, :, None],
    [[383, 285], [474, 209], [579, 229], [542, 270]],
    [[385, 280], [475, 208], [577, 230], [539, 272]])
bracelet = register(atlas * rect(525, 490, 568, 524)[:, :, None],
    [[532, 500], [552, 502], [549, 515]], [[535, 500], [555, 502], [550, 515]])
butterfly_masks = [rect(204, 691, 251, 743), rect(642, 665, 702, 719), rect(656, 822, 702, 863)]
petals_owner = np.maximum(rect(250, 633, 308, 687), rect(730, 629, 787, 696))
legs = polygon([(449, 631), (557, 605), (605, 774), (610, 851), (554, 854),
    (545, 785), (499, 738), (488, 837), (490, 982), (447, 985), (443, 836)])
garden_domain = rect(204, 628, 818, 1023) * (1 - petals_owner)
for mask in butterfly_masks:
    garden_domain *= 1 - mask
garden_visible = garden_domain * (1 - legs)

# Resolve source ownership once. An exposed layer is never alpha-over restored
# onto another copy of the same source edge, which would thicken antialiasing.
claimed = np.zeros((1024, 1024), np.float32)
owners = {}
sleeve_owner = polygon([(532, 370), (559, 369), (575, 379), (567, 395),
    (551, 407), (533, 417), (531, 401)])
for name, mask in [('sleeve', sleeve_owner), ('bracelet', bracelet_owner), ('right-arm', right_owner),
                   ('hat-front', hat_owner), ('left-arm', left_owner),
                   ('bouquet', bouquet_owner), ('hair', hair_owner),
                   ('ribbon', train_owner), ('petals', petals_owner),
                   *[(f'butterfly-{i + 1}', mask) for i, mask in enumerate(butterfly_masks)],
                   ('garden', garden_visible)]:
    owners[name] = mask * (1 - claimed)
    claimed = np.maximum(claimed, mask)
static = 1 - claimed
body_fill = hidden_torso * np.maximum(owners['right-arm'], owners['hair']) * opaque
body = over(body * body_fill[:, :, None], source * static[:, :, None])
hair_fill = hidden_hair * static * opaque * rect(400, 295, 538, 487)
hair = over(hair * hair_fill[:, :, None], source * owners['hair'][:, :, None])
left_fill = polygon([(413, 405), (438, 412), (437, 435), (414, 438)]) * static * opaque
left = over(left * left_fill[:, :, None], source * owners['left-arm'][:, :, None])
right_fill = np.maximum(rect(540, 386, 568, 414) * np.maximum(static, owners['sleeve']), owners['bracelet']) * opaque
right = over(right * right_fill[:, :, None], source * owners['right-arm'][:, :, None])
hat_front = source * owners['hat-front'][:, :, None]
hat_hidden = polygon([(429, 290), (458, 257), (491, 237), (519, 247), (537, 278), (526, 299)])
hat_back = hat * (hat_hidden * static * opaque)[:, :, None]
bouquet_hidden = rect(338, 561, 373, 587) * owners['left-arm'] * opaque
bouquet = over(atlas * bouquet_hidden[:, :, None], source * owners['bouquet'][:, :, None])
bracelet_back = bracelet * (rect(533, 494, 561, 518) * owners['right-arm'] * opaque)[:, :, None]
bracelet = source * owners['bracelet'][:, :, None]
train_hidden = polygon([(580, 488), (631, 480), (674, 503), (667, 531), (603, 541)]) * static * opaque
train = over(atlas * train_hidden[:, :, None], source * owners['ribbon'][:, :, None])
garden = over(atlas * (garden_domain * legs * opaque)[:, :, None], source * owners['garden'][:, :, None])

save('original.png', source)
save('garden.png', garden)
save('body.png', body)
parts = []
channels = {}
actions = {}
layers = []


def part(name, pixels, pivot, foreground=False, channel=None, action=None, idle=None):
    save(name + '.png', pixels)
    coverage = np.repeat(pixels[:, :, 3:4], 4, axis=2)
    coverage[:, :, 3] = 1
    save(name + '-mask.png', coverage)
    spec = {'image': name + '.png', 'mask': name + '-mask.png',
            'pivot': [v / 2 for v in pivot], 'foreground': foreground, 'fill_background': False}
    if idle is not None:
        spec['idle'] = idle
    else:
        spec.update(channel=channel, action=action)
    parts.append(spec)
    layers.append((name, pixels))


for name, angle, duration in [('bouquet', -2.2, 2.8), ('arm', 1.8, 2.6),
                               ('hat', -1.8, 2.4), ('ribbon', 2.6, 3.2), ('butterflies', 9., 1.8)]:
    channel = name + '_angle'
    channels[channel] = {'default': 0, 'range': [min(0, angle), max(0, angle)]}
    actions[name] = {'duration': duration, 'tracks': {channel: [
        [0, 0, 'smooth'], [duration * .35, angle, 'smooth'],
        [duration * .6, angle, 'smooth'], [duration, 0, 'linear']]}}

part('ribbon', train, [650, 490], channel='ribbon_angle', action='ribbon')
part('hair', hair, [447, 303], idle={'period': 6, 'angle': 2.})
part('hat-back', hat_back, [488, 256], channel='hat_angle', action='hat')
part('bouquet', bouquet, [426, 422], channel='bouquet_angle', action='bouquet')
part('left-arm', left, [426, 422], channel='bouquet_angle', action='bouquet')
for i, mask in enumerate(butterfly_masks):
    points = np.argwhere(mask > 0)
    pivot = [float(points[:, 1].mean()), float(points[:, 0].mean())]
    part('butterfly-' + str(i + 1), source * mask[:, :, None], pivot,
         channel='butterflies_angle', action='butterflies')
part('petals', source * petals_owner[:, :, None], [490, 657], idle={'period': 6, 'angle': 1.2})
part('bracelet-back', bracelet_back, [550, 393], foreground=True, channel='arm_angle', action='arm')
part('right-arm', right, [550, 393], foreground=True, channel='arm_angle', action='arm')
part('bracelet', bracelet, [550, 393], foreground=True, channel='arm_angle', action='arm')
part('hat-front', hat_front, [488, 256], foreground=True, channel='hat_angle', action='hat')
# Original sleeve overlays the generated shoulder cap; it does not rotate with skin.
shoulder = source * owners['sleeve'][:, :, None]
part('sleeve', shoulder, [547, 385], foreground=True, idle={'period': 6, 'angle': 0})

model = {'format': 'lexington-layered-actor-1', 'canvas': [512, 512],
         'original': 'original.png', 'background': 'garden.png', 'foreground': 'body.png',
         'fixed_background': True, 'breath_body_only': True, 'root_pivot': [237.5, 479],
         'sway': {'period': 6, 'angle': .12, 'dx': .18},
         'breath': {'period': 3, 'center': [243, 211], 'radii': [32, 34], 'dx_scale': .008, 'dy': -.65},
         'channels': channels, 'actions': actions, 'parts': parts}
(MODEL / 'actor.json').write_text(json.dumps(model, ensure_ascii=False, indent=2) + '\n')

# A technical assembly preview; final motion review is rendered by the native Rust actor.
neutral = garden.copy()
for spec, (_, pixels) in zip(parts, layers):
    if not spec['foreground']:
        neutral = over(neutral, pixels)
neutral = over(neutral, body)
for spec, (_, pixels) in zip(parts, layers):
    if spec['foreground']:
        neutral = over(neutral, pixels)
save('neutral.png', neutral)
preview = Image.new('RGB', (512 * 2, 512), (70, 75, 85))
for i, name in enumerate(['original.png', 'neutral.png']):
    im = Image.open(MODEL / name)
    preview.paste(im, (i * 512, 0), im)
preview.save(DRAFT / 'assembly-review.png')

if args.publish:
    LIVE.mkdir(parents=True, exist_ok=True)
    referenced = {model['original'], model['background'], model['foreground']}
    for spec in parts:
        referenced.update([spec['image'], spec['mask']])
    for name in sorted(referenced):
        shutil.copy2(MODEL / name, LIVE / name)
    temporary = LIVE / '.actor.json.tmp'
    shutil.copy2(MODEL / 'actor.json', temporary)
    temporary.replace(LIVE / 'actor.json')
print('Published' if args.publish else 'Draft', LIVE if args.publish else MODEL)
