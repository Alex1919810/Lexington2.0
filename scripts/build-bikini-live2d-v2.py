"""Offline full-layer authoring; NumPy/OpenCV/Pillow never enter the app runtime.

Consumes generated/*.png and measured alignment.json. Default output is a draft
model; --publish copies reviewed files into the native outfit with actor.json last.
"""
import argparse
import json
import math
from pathlib import Path
import shutil

import cv2
import numpy as np
from PIL import Image, ImageDraw

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--publish',action='store_true')
args=parser.parse_args()
ROOT=Path(__file__).resolve().parents[1]
DRAFT=ROOT/'resource/illustration/lexington/live2d-drafts/BiKiNi_Normal_v2'
LIVE=ROOT/'resource/illustration/lexington/live2d/BiKiNi_Normal'
MODEL=DRAFT/'model';MODEL.mkdir(parents=True,exist_ok=True)
OWN=DRAFT/'ownership'
required=[DRAFT/'alignment.json',*[DRAFT/'generated'/(name+'.png') for name in ['board','hair','body','arm','hat']],
          *[OWN/(name+'.png') for name in ['board','hair','hat','body']]]
missing=[str(path.relative_to(ROOT)) for path in required if not path.is_file()]
if missing:
    parser.error('缺少本地临时制作源（不随 Git 分发）：'+', '.join(missing)+
                 '；运行程序直接使用 live2d/BiKiNi_Normal 正式资源，无需重建。')
alignment=json.loads((DRAFT/'alignment.json').read_text())
# Positive area filtering: Lanczos' negative lobes created detached 1..4 alpha
# rings around legs and hair, with extreme RGB after unpremultiplication.
def area_image(path):
    pixels=np.array(Image.open(path).convert('RGBA')).astype(np.float32)/255
    pixels[:,:,:3]*=pixels[:,:,3:4]
    pixels=cv2.resize(pixels,(512,512),interpolation=cv2.INTER_AREA)
    alpha=pixels[:,:,3:4]
    pixels[:,:,:3]=np.divide(pixels[:,:,:3],alpha,out=np.zeros_like(pixels[:,:,:3]),where=alpha>1e-7)
    pixels=np.uint8(np.clip(pixels*255+.5,0,255));pixels[pixels[:,:,3]==0]=0
    return pixels

original=area_image(ROOT/'resource/illustration/lexington/png/BiKiNi_Normal.png')

def save_rgba(name,array):
    array=np.uint8(np.clip(array,0,255));array[array[:,:,3]==0]=0
    Image.fromarray(array).save(MODEL/name)

def aligned_image(name):
    image=area_image(DRAFT/'generated'/(name+'.png')).astype(np.float32)/255
    image[:,:,:3]*=image[:,:,3:4]
    record=alignment[name]
    if 'transform' in record:
        image=cv2.warpAffine(image,np.float32(record['transform']),(512,512),flags=cv2.INTER_LINEAR)
    else:
        controls=np.float64([p[1] for p in record['inverse_tps_controls']])
        source=np.float64([p[0] for p in record['inverse_tps_controls']])
        delta=controls[:,None,:]-controls[None,:,:];r2=(delta*delta).sum(2);p=np.c_[np.ones(len(controls)),controls]
        system=np.block([[r2*np.log(r2+1e-12)+np.eye(len(controls))*.001,p],[p.T,np.zeros((3,3))]])
        coeff=np.linalg.solve(system,np.r_[source,np.zeros((3,2))])
        yy,xx=np.mgrid[:512,:512];locations=np.c_[xx.ravel(),yy.ravel()].astype(np.float64)
        delta=locations[:,None,:]-controls[None,:,:];r2=(delta*delta).sum(2)
        uv=(r2*np.log(r2+1e-12))@coeff[:len(controls)]+np.c_[np.ones(len(locations)),locations]@coeff[len(controls):]
        image=cv2.remap(image,uv[:,0].reshape(512,512).astype(np.float32),uv[:,1].reshape(512,512).astype(np.float32),cv2.INTER_LINEAR,borderMode=cv2.BORDER_CONSTANT)
    alpha=image[:,:,3:4]
    image[:,:,:3]=np.divide(image[:,:,:3],alpha,out=np.zeros_like(image[:,:,:3]),where=alpha>1e-7)
    image=np.uint8(np.clip(image*255+.5,0,255))
    # Remove isolated generator dust; retain a 3px soft fringe around the main
    # generated object. Original visible pixels are restored afterward, unfiltered.
    count,labels,stats,_=cv2.connectedComponentsWithStats((image[:,:,3]>8).astype(np.uint8))
    main=1+int(np.argmax(stats[1:,cv2.CC_STAT_AREA]))
    support=cv2.dilate((labels==main).astype(np.uint8),np.ones((7,7),np.uint8))
    image[support==0]=0
    return image

board=aligned_image('board');hair=aligned_image('hair');body=aligned_image('body');arm=aligned_image('arm');hat=aligned_image('hat')
crown_plate=body.copy()
board_owner=np.array(Image.open(OWN/'board.png').convert('L'))>0
hair_owner=np.array(Image.open(OWN/'hair.png').convert('L'))>0
hat_owner=np.array(Image.open(OWN/'hat.png').convert('L'))>0
body_owner=np.array(Image.open(OWN/'body.png').convert('L'))>0

# One continuous arm texture, registered to the original bones. Generated
# shoulder and wrist anatomy supplies the previously hidden regions.
arm_domain=Image.new('L',(512,512))
ImageDraw.Draw(arm_domain).polygon([(270,198),(266,182),(277,173),(297,163),(289,139),
                                  (278,123),(297,123),(306,143),(319,162),(321,174),(292,190)],fill=255)
arm_domain=np.array(arm_domain)>0
r,g,b=[original[:,:,i].astype(np.int16) for i in range(3)]
# Keep the generated arm as one painted surface. Pasting the old skin through
# a slightly different arm silhouette creates a second, internal paint seam.
# Keep generated support local to the measured complete arm, not stray regions.
arm[:118]=0;arm[207:]=0;arm[:,:260]=0;arm[:,327:]=0
# The generated arm also contains dark cap/background scraps beside the elbow.
# Keep the continuous skin surface plus its one-pixel antialiasing contour.
ar,ag,ab=[arm[:,:,i].astype(np.int16) for i in range(3)]
skin=((arm[:,:,3]>32)&(ar>170)&(ag>95)&(ab>80)&(ar>ag+10)&(ar>ab+15)).astype(np.uint8)
distance=cv2.distanceTransform(1-skin,cv2.DIST_L2,5)
arm[distance>1.4]=0


# The original face, clothing, body and front framing hair remain unchanged in
# their visible core. The complete static plate supports newly revealed shoulder.
body_owner &= ~arm_domain
body_owner[174:219,267:294]=False
near_body=cv2.dilate((body[:,:,3]>32).astype(np.uint8),np.ones((3,3),np.uint8))>0
body_owner &= near_body
cyan=(b>r+7)&(g>r+7)
body_owner[228:330,185:310] &= ~cyan[228:330,185:310]
source_body=original.copy();source_body[:,:,3]=np.where(body_owner,original[:,:,3],0)
body=np.array(Image.alpha_composite(Image.fromarray(body),Image.fromarray(source_body)))
# Preserve the original facial contour as RGBA, not alpha-over: compositing the
# generated face beneath it would thicken the original jaw anti-aliasing.
face=np.zeros((512,512),bool);face[143:176,225:267]=True
face &= ~hat_owner & ~arm_domain
body[face]=original[face]

# The clean plate's rounded amputated shoulder is only an underpainting.
# Its exposed silhouette must end at the measured torso/rig attachment, rather
# than forming a second stationary upper arm alongside the skinned arm.
for y in range(174,208):
    edge=float(np.interp(y,[174,181,193,207],[271,276,278,280]))
    body[y,int(math.ceil(edge))+1:294]=0


# The old hair selection included a triangle of upper-arm skin. Ownership
# must exclude the complete arm and static clothing before restoring source RGB.
hair_owner &= ~arm_domain & ~body_owner

# Generated parts provide all occluded pixels; original visible artwork wins.
# Outside the ownership domains, no copy/dilation strip is used as hidden texture.
# The complete board keeps a single coherent generated surface. Splicing its
# pattern at the old body cut would expose a paint seam when the board rotates.
weight=np.minimum(1,cv2.distanceTransform(hair_owner.astype(np.uint8),cv2.DIST_L2,3)/8)[:,:,None]
hair[:,:,:3]=np.uint8(hair[:,:,:3]*(1-weight)+original[:,:,:3]*weight+.5)
hair[:,:,3]=np.where(hair_owner,original[:,:,3],hair[:,:,3])
hair[:,:258]=0;hair[258:]=0
# This is BACK hair only. Generated scalp above the real back-hair root
# must not protrude above the stable head or cover the moving cap.
root_hair=hair.copy()
hair[:160]=0
# Measured original hair/cap contact, not the taller generated scalp outline.
# Keep a small hidden 2px crown overlap; the cap's geometric front region covers it.
contact_x=[220,224,229,234,239,245,250,255,260,265,270,276,282]
contact_y=[144,133,126,122,119,120,123,126,129,133,137,141,146]
yy,xx=np.mgrid[:512,:512]
contact=np.interp(xx,contact_x,contact_y)
head=(xx>=220)&(xx<=282)&(yy<180)
# Stable root stays behind the head/arm and overlaps the moving back locks.
# Root coverage follows the back of the cranium, not the neutral cap rim.
# The old constant y147 cutoff made a rectangular opening above the temple.
back_top=np.interp(xx,[258,270,282,292,304],[127,134,139,144,153])
root_hair[(xx<258)|(xx>304)|(yy<back_top-0.5)|(yy>=178)]=0
root_coverage=np.clip(yy-back_top+0.5,0,1)
root_hair[:,:,3]=np.uint8(root_hair[:,:,3]*root_coverage)
solid=(root_hair[:,:,3]>=240)&(yy>back_top+1)
root_hair[solid,3]=255
# The generated plate still has a small old-hand matte hollow at the temple.
# Extend neighboring painted root texture only inside the measured head-side
# support polygon, with area-sampled AA at its free contour.
guard=Image.new('L',(2048,2048))
ImageDraw.Draw(guard).polygon([(278*4,138*4),(284*4,139*4),(292*4,144*4),
                              (295*4,151*4),(292*4,157*4),(280*4,156*4)],fill=255)
guard=np.array(guard.resize((512,512),Image.Resampling.BOX))
valid=(root_hair[:,:,3]>=240).astype(np.uint8)
_,nearest=cv2.distanceTransformWithLabels(1-valid,cv2.DIST_L2,5,labelType=cv2.DIST_LABEL_PIXEL)
colors=np.zeros((int(nearest.max())+1,3),np.uint8)
colors[nearest[valid>0]]=root_hair[valid>0,:3]
fill=guard>root_hair[:,:,3]
old_alpha=root_hair[fill,3].astype(np.float32)
new_alpha=guard[fill].astype(np.float32)
weight=(old_alpha/np.maximum(1,new_alpha))[:,None]
root_hair[fill,:3]=np.uint8(root_hair[fill,:3]*weight+colors[nearest[fill]]*(1-weight)+.5)
root_hair[fill,3]=guard[fill]

# Use the existing complete painted crown, not a contact-line cutout. Its
# superior silhouette is lowered 1px to match the original head apex. One
# continuous painted surface removes the old step between two crown textures.
plate=np.zeros_like(crown_plate);plate[1:]=crown_plate[:-1]
# Generated interior hair used alpha 252/253; it is an opaque material,
# not a translucent veil through which the rear brim should show.
plate[plate[:,:,3]>=240,3]=255
crown=head&(yy<143)
old=body.copy()
body[crown]=plate[crown]
# Preserve the original face and visible bangs below the crown transition.
u=np.clip((yy-134)/9,0,1)[:,:,None]
body[:,:,:3]=np.where(crown[:,:,None],body[:,:,:3]*(1-u)+old[:,:,:3]*u,body[:,:,:3]).astype(np.uint8)
v=np.clip((yy-130)/6,0,1)
body[:,:,3]=np.where(crown,body[:,:,3]*(1-v)+old[:,:,3]*v,body[:,:,3]).astype(np.uint8)

# Depth ownership is a continuous REGION, independent of cap alpha and generated
# scalp alpha. The woven holes must not punch holes in the depth mask itself.
cap_edge=np.interp(xx,contact_x,[144,133,126,122,119,116,120,123,127,130,134,139,144])
front_region=((yy<cap_edge)&(xx<=282))|((xx>282)&(yy<151))
front_region &= (yy<155)
# Flower and hand touch: color thresholds confuse the yellow flower center
# with skin. Use the measured finger silhouette, never a skin-color cutout.
fingers=Image.new('L',(512,512))
ImageDraw.Draw(fingers).polygon([(287,131),(291,129),(295,132),(300,140),
                                (301,151),(291,152),(285,139)],fill=255)
fingers=np.array(fingers)>0
flower=((xx-285)/14)**2+((yy-124)/11)**2<1
visible_hat=hat_owner&front_region&(~arm_domain|flower)&~fingers
# Do not splice even a one-pixel strip of original hair onto the moving cap.
# Mixed cap/hair boundary pixels belong to neither pure texture. Use the
# complete cap painting throughout a 4px contact band, instead of carrying
# the original hair fringe upward with the hat.
visible_hat[head&((yy>=cap_edge-4)|(r<=b+25))]=False
hat[visible_hat]=original[visible_hat]
hat[(arm[:,:,3]>200)&(yy<151)]=0
scalp=head&(yy>=contact)&(yy<180)
hat_support=front_region|scalp
hat[~hat_support]=0
front_weight=np.where(xx<=282,np.clip(cap_edge-yy+0.5,0,1),np.clip(151-yy+0.5,0,1))
front=np.uint8(np.where(yy<155,front_weight,0)*255+.5)

# Preserve the original free silhouette instead of combining two slightly
# different hair contours. Generated texture is only the hidden root support.
free=(xx>=280)&(yy>=175)
hidden=arm_domain|(body[:,:,3]>=250)
hair[free&~hidden&~hair_owner]=0
source_free=free&hair_owner
hair[source_free,3]=original[source_free,3]
# Hidden painted hair behind the old arm is solid; generator matte coverage
# must not leave a translucent arm-shaped hollow after the arm moves.
hair[arm_domain&(hair[:,:,3]>=128),3]=255
# Outside the original opaque scene, a generated underplate must never
# thicken the source antialiasing. This includes breast and waist exteriors.
body[190:330,:,3]=np.minimum(body[190:330,:,3],original[190:330,:,3])
# On the right torso, hair/board behind the skin makes source alpha opaque.
# Trace the measured source skin edge and dematte its last pixel instead of
# retaining the wider generated shoulder/board-colored fringe.
edge_rows=[193,195,200,205,208,210,215,220,225,230,235,240,245,250,255,260,265,270]
edge_cols=[279,278,277,277,277,277,276,275,273,272,270,269,271,272,273,274,276,278]
for y in range(193,271):
    expected=int(round(float(np.interp(y,edge_rows,edge_cols))))
    candidates=[x for x in range(expected-2,expected+2)
                if r[y,x]>190 and r[y,x]>g[y,x]+12 and r[y,x]>b[y,x]+24]
    edge=max(candidates) if candidates else expected
    body[y,edge+1:320]=0
    if original[y,edge+2,3]==255:
        c=original[y,edge-1,:3].astype(np.float32)
        back=original[y,edge+2,:3].astype(np.float32)
        pixel=original[y,edge,:3].astype(np.float32)
        delta=c-back
        coverage=float(np.clip(np.dot(pixel-back,delta)/max(1,float(np.dot(delta,delta))),0,1))
        body[y,edge,:3]=c.astype(np.uint8)
        body[y,edge,3]=round(255*coverage)
    else:
        body[y,edge]=original[y,edge]

# Below all moving objects, use the exact original RGBA, including antialiasing.
body[330:]=original[330:]

# Remove detached dust only. Do not open/threshold the entire silhouette: that
# turns thin antialiased edges into an alternating series of hard steps.
edge_report={}
for name,layer in [('body',body),('arm',arm),('board',board),('hair',hair),('hat',hat)]:
    before=layer[:,:,3].copy()
    count,labels,stats,_=cv2.connectedComponentsWithStats((before>0).astype(np.uint8))
    for i in range(1,count):
        if stats[i,cv2.CC_STAT_AREA]<4:layer[labels==i]=0
    edge_report[name]={'detached_pixels_removed':int(np.count_nonzero((before>0)&(layer[:,:,3]==0)))}

save_rgba('board.png',board);save_rgba('hair.png',hair);save_rgba('body.png',body)
save_rgba('arm.png',arm);save_rgba('hat.png',hat);save_rgba('original.png',original)
save_rgba('transparent.png',np.zeros_like(original))
save_rgba('roots.png',root_hair)
Image.fromarray(front).convert('RGBA').save(MODEL/'hat-front-mask.png')
for name,image in [('board',board),('hair',hair),('arm',arm),('hat',hat)]:
    Image.fromarray(image[:,:,3]).convert('RGBA').save(MODEL/(name+'-mask.png'))

# Hair is a single rigid, unit-scale surface. Its cut/root is hidden behind
# the stable head/body; no spatially varying rotation weights squeeze the locks.

shoulder=np.array([275.,188.]);elbow=np.array([308.,167.]);wrist=np.array([295.,137.])
upper_length=float(np.linalg.norm(elbow-shoulder));lower_length=float(np.linalg.norm(wrist-elbow))
arm_vertices=[];arm_indices=[];xs_arm=list(range(256,333,4));ys_arm=list(range(116,213,4))
for y in ys_arm:
    for x in xs_arm:
        point=np.array([float(x),float(y)])
        tangent=(elbow-shoulder)/upper_length+(wrist-elbow)/lower_length
        tangent=tangent/np.linalg.norm(tangent)
        signed=float(np.dot(point-elbow,tangent))
        low=max(0,min(1,(signed+8)/16));low=low*low*(3-2*low)
        along=float(np.dot(point-shoulder,(elbow-shoulder)/upper_length))
        root=max(0,min(1,along/10));root=1-root*root*(3-2*root)
        root=(1-low)*root
        upper=1-root-low
        arm_vertices.append([x,y,round(root,7),round(upper,7),round(low,7)])
for row in range(len(ys_arm)-1):
    for col in range(len(xs_arm)-1):
        a=row*len(xs_arm)+col;b=a+1;c=a+len(xs_arm);d=c+1
        x,y=xs_arm[col],ys_arm[row]
        if np.any(arm[max(0,y-2):min(512,y+7),max(0,x-2):min(512,x+7),3]):
            arm_indices.extend([a,b,d,a,d,c])

channels={'board_angle':{'default':0,'range':[-3.5,3.5]},'hat_angle':{'default':0,'range':[0,6]}}
actions={
 'board':{'duration':3.2,'tracks':{'board_angle':[[0,0,'smooth'],[.8,3.5,'smooth'],[2.2,-3.5,'smooth'],[3.2,0,'linear']]}},
 'hat':{'duration':2.8,'tracks':{'hat_angle':[[0,0,'smooth'],[.9,6,'linear'],[1.6,6,'smooth'],[2.8,0,'linear']]}}}
model={'format':'lexington-layered-actor-1','canvas':[512,512],'original':'original.png','background':'roots.png',
 'foreground':'body.png','fixed_background':True,'breath_body_only':True,'background_above_depth':True,'root_pivot':[259,497.5],
 'sway':{'period':6,'angle':.16,'dx':.275},
 'breath':{'period':3,'center':[244,236.5],'radii':[29,38],'dx_scale':.009,'dy':-.9},
 'rig':{'channel':'hat_angle','bones':[shoulder.tolist(),elbow.tolist(),wrist.tolist()],'pole':1,'target_delta':[0,-8/6]},
 'channels':channels,'actions':actions,'parts':[
 {'image':'board.png','mask':'board-mask.png','pivot':[202,287],'channel':'board_angle','action':'board','foreground':False},
 {'image':'hair.png','mask':'hair-mask.png','pivot':[258,145],'idle':{'period':6,'angle':6},'fill_background':False,'foreground':False},
 {'image':'hat.png','mask':'hat-mask.png','front_mask':'hat-front-mask.png','pivot':wrist.tolist(),'channel':'hat_angle','action':'hat','binding':'lower','foreground':False},
 {'image':'arm.png','mask':'arm-mask.png','pivot':shoulder.tolist(),'channel':'hat_angle','action':'hat','foreground':False,
  'mesh':{'kind':'arm','vertices':arm_vertices,'indices':arm_indices}}]}
encoded=json.dumps(model,separators=(',',':'))+'\n'
assert len(encoded.encode())<=65536
(MODEL/'actor.json').write_text(encoded)
metrics={'canvas':[512,512],'model_bytes':len(encoded.encode()),'arm_vertices':len(arm_vertices),'hair_transform':'rigid_unit_scale',
         'original_foreground_pixels':int(np.count_nonzero(body_owner)),'generated_canvas':[1254,1254],
         'edge_cleanup':edge_report,'runtime':'native Rust; one connected arm texture; shared-vertex meshes; one hat resample with depth interpolation'}
(DRAFT/'build-report.json').write_text(json.dumps(metrics,indent=2)+'\n')
sheet=Image.new('RGB',(512*5,540),'#526071')
for i,name in enumerate(['board','hair','body','arm','hat']):
    image=Image.open(MODEL/(name+'.png'));sheet.paste(image,(i*512,28),image);ImageDraw.Draw(sheet).text((i*512+8,8),name,fill='white')
sheet.save(DRAFT/'complete-layers.png')
if args.publish:
    for path in MODEL.iterdir():
        if path.is_file() and path.name!='actor.json':shutil.copyfile(path,LIVE/path.name)
    temporary=LIVE/'actor.json.v2-tmp';shutil.copyfile(MODEL/'actor.json',temporary);temporary.replace(LIVE/'actor.json')
print(json.dumps(metrics))
