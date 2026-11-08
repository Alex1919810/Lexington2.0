"""Offline conversion of the supplied unified actor; never used by the application."""
import argparse
import io
import json
from pathlib import Path
import zipfile

import numpy as np
from PIL import Image


def convert(archive, destination):
    destination = Path(destination)
    if destination.exists():
        raise ValueError('Destination already exists')
    with zipfile.ZipFile(archive) as package:
        source = json.loads(package.read('actor.json'))
        rig = np.load(io.BytesIO(package.read('assets/reference-rig-state.npz')), allow_pickle=False)
        hair = np.load(io.BytesIO(package.read('assets/hair-layer-mask.npz')), allow_pickle=False)
        destination.mkdir(parents=True)
        assets = destination / 'assets'
        assets.mkdir()

        def png(name, array, mask=False):
            array = np.asarray(array)
            if mask:
                array = np.rint(np.clip(array, 0, 1) * 65535).astype(np.uint16)
            else:
                array = array.copy()
                if array.ndim == 3 and array.shape[2] == 4:
                    array[array[..., 3] == 0, :3] = 0
            path = 'assets/' + name + '.png'
            Image.fromarray(array).save(destination / path)
            return path

        def layer(name, array):
            ys, xs = np.nonzero(array[..., 3])
            x, y, r, b = int(xs.min()), int(ys.min()), int(xs.max())+1, int(ys.max())+1
            return {'path': png(name, array[y:b, x:r]), 'origin': [x, y]}

        original = png('original', rig['original'])
        plate = png('plate', rig['plate'])
        layers = [layer(n, rig[n]) for n in ['left_upper', 'right_upper', 'left_lower', 'right_lower', 'bouquet']]
        masks = [png('shoulder', rig['shoulder_restore'], True), png('bodice', rig['front_mask']*(1-rig['bouquet_mask']), True)]
        for key in ['breath_dx', 'breath_dy']:
            rig[key].astype('<f4').tofile(assets / (key+'.f32'))
        roi = [52, 150, 228, 360]
        x, y, r, b = roi
        front = np.dstack((hair['front_rgb']*hair['front_alpha'][..., None], hair['front_alpha']))
        front[y:b, x:r].astype('<f4').tofile(assets / 'hair-front.f32')
        hair_mask = png('hair-mask', hair['mask'][y:b, x:r], True)
        inner = png('hair-occlusion', hair['inner_occlusion'][y:b, x:r].astype(np.uint8)*255)
        for name in ['eye-atlas-33.png', 'eye-cover-mask.png']:
            (assets/name).write_bytes(package.read('assets/'+name))
        actions = {
            'blink': {'duration': .64, 'tracks': {'eye_openness': [[0, 1, 'smooth'], [.26, 0, 'linear'], [.36, 0, 'smooth'], [.64, 1, 'linear']]}},
            'bouquet': {'duration': 3.8, 'tracks': {'bouquet_lift_px': [[0, 0, 'quintic_squared'], [1.5, 10, 'linear'], [2.1, 10, 'quintic_complement_squared'], [3.8, 0, 'linear']]}}
        }
        model = {
            'format': 'lexington-native-actor-1', 'canvas': source['canvas'],
            'original': original, 'plate': plate, 'layers': layers, 'restore_masks': masks,
            'bones': {'left': source['bones']['left'], 'right': source['bones']['right'], 'poles': [1, -1]},
            'head_until': 190, 'breath_roi': [145, 190, 340, 375],
            'breath_fields': ['assets/breath_dx.f32', 'assets/breath_dy.f32'],
            'hair_roi': roi, 'hair_mask': hair_mask, 'hair_occlusion': inner,
            'hair_front': 'assets/hair-front.f32', 'hair_body_occlusion': [200, 195, 355],
            'shared_idle': source['shared_idle'], 'root_pivot': source['root_pivot'],
            'channels': source['channels'], 'actions': actions,
            'hit_regions': source['hit_regions'],
            'eyes': {'atlas': 'assets/eye-atlas-33.png', 'mask': 'assets/eye-cover-mask.png',
                     'cell': [89, 92], 'columns': 8, 'states': 33, 'origin': [218, 99]},
        }
        (destination/'actor.json').write_text(json.dumps(model, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
        (destination/'source-actor.json').write_bytes(package.read('actor.json'))
        (destination/'README.txt').write_text('User-supplied Lexington WeddingDress unified character hair v4.\nNative runtime data converted from the supplied NumPy rig and PNG assets.\nNot a Cubism moc3 model. Python is used only for offline conversion.\n', encoding='utf-8')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('archive')
    parser.add_argument('destination')
    args = parser.parse_args()
    convert(args.archive, args.destination)
