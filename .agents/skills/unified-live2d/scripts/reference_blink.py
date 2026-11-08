"""Reference-shaped eyelids: stable iris, real closed-keyframe lash texture."""
import cv2
import numpy as np
from copy import deepcopy

class ReferenceBlink:
    def __init__(self, opened, closed, rig, opened_alpha=None, closed_alpha=None):
        if opened.shape != closed.shape or opened.ndim != 3 or opened.shape[2] != 3:
            raise ValueError('Open/closed references must be RGB images of identical dimensions')
        if not isinstance(rig,dict) or not {'face_box','eyes'}<=rig.keys():
            raise ValueError('Rig must be an object containing face_box and eyes')
        if 'canvas_size' in rig and rig['canvas_size'] != [opened.shape[1],opened.shape[0]]:
            raise ValueError('canvas_size differs from the aligned input canvas; transform all rig coordinates')
        if not isinstance(rig['face_box'],(list,tuple)):
            raise ValueError('face_box requires four integer pixel coordinates')
        box=tuple(rig['face_box'])
        if len(box)!=4 or not all(isinstance(v,int) and not isinstance(v,bool) for v in box):
            raise ValueError('face_box requires four integer pixel coordinates')
        self.box=box
        fx,fy,fr,fb=box
        if not (0<=fx<fr<=opened.shape[1] and 0<=fy<fb<=opened.shape[0]):
            raise ValueError('face_box must lie within the aligned input canvas')
        self.size=(fr-fx,fb-fy)
        self.s=4
        self.specs=deepcopy(rig['eyes'])
        if not isinstance(self.specs,list) or not self.specs:
            raise ValueError('At least one visible eye must be configured')
        for eye in self.specs:
            required={'x','upper','lower','poly','closed_search'}
            if not isinstance(eye,dict) or not required<=eye.keys():
                raise ValueError('Each eye requires x, upper, lower, poly, closed_search')
            if not all(isinstance(eye[k],list) for k in required):
                raise ValueError('Eye guides, polygon and closed_search must be JSON arrays')
            if not all(isinstance(v,(int,float)) and not isinstance(v,bool) and np.isfinite(v) for k in ('x','upper','lower') for v in eye[k]):
                raise ValueError('Eye guides must contain finite numeric coordinates')
            if not all(isinstance(point,list) and len(point)==2 and all(isinstance(v,(int,float)) and not isinstance(v,bool) and np.isfinite(v) for v in point) for point in eye['poly']):
                raise ValueError('Eye polygon points must each contain two finite numbers')
            n=len(eye['x'])
            guides=np.array(eye['x']+eye['upper']+eye['lower'],dtype=float)
            if n<3 or n!=len(eye['upper']) or n!=len(eye['lower']) or not np.isfinite(guides).all() or np.any(np.diff(eye['x'])<=0):
                raise ValueError('Eye x/upper/lower guides need equal lengths and increasing x')
            polygon=np.array(eye['poly'],dtype=float)
            if polygon.ndim!=2 or polygon.shape[1]!=2 or len(polygon)<3 or not np.isfinite(polygon).all():
                raise ValueError('Eye poly must contain at least three finite x/y points')
            if not all(fx<=x<fr and fy<=y<fb for x,y in eye['poly']):
                raise ValueError('Eye polygons must lie inside face_box')
            if not fx<=eye['x'][0]<eye['x'][-1]<fr:
                raise ValueError('Eye x guide must lie inside face_box')
            if not all(fy<=v<fb for v in eye['upper']+eye['lower']):
                raise ValueError('Open eyelid y guides must lie inside face_box')
            if len(eye['closed_search'])!=2 or not all(isinstance(v,(int,float)) and np.isfinite(v) for v in eye['closed_search']):
                raise ValueError('closed_search needs two finite y coordinates')
            if not fy<=eye['closed_search'][0]<eye['closed_search'][1]<=fb:
                raise ValueError('closed_search must lie inside face_box')
        matte=rig.get('matte',{})
        if not isinstance(matte,dict):
            raise ValueError('matte must be an object')
        try:
            closed_dark=float(matte.get('closed_dark_threshold',165))
            open_dark=float(matte.get('open_dark_threshold',170))
            red_margin=float(matte.get('open_red_margin',-8))
        except (ValueError,TypeError) as error:
            raise ValueError('matte thresholds must be finite numbers') from error
        if not np.isfinite([closed_dark,open_dark,red_margin]).all() or not (0<=closed_dark<=255 and 0<=open_dark<=255 and -255<=red_margin<=255):
            raise ValueError('matte dark thresholds must be in [0,255]; red margin in [-255,255]')
        self.original=opened[fy:fb,fx:fr].copy()
        other=closed[fy:fb,fx:fr].copy()
        registration_open=self.original
        registration_closed=other
        closed_crop_alpha=None
        if opened_alpha is not None or closed_alpha is not None:
            for alpha in (opened_alpha,closed_alpha):
                if alpha is None or alpha.shape!=opened.shape[:2] or not np.isfinite(alpha).all() or alpha.min()<0 or alpha.max()>1:
                    raise ValueError('Reference alpha planes must match the canvas and lie in [0,1]')
            open_crop_alpha=opened_alpha[fy:fb,fx:fr,None]
            closed_crop_alpha=closed_alpha[fy:fb,fx:fr,None]
            # Neutral compositing is for registration observations only. Color
            # synthesis remains straight RGB and never contains a preview matte.
            neutral=np.array([24,36,46],dtype=np.float32)
            registration_open=np.uint8(np.rint(self.original*open_crop_alpha+neutral*(1-open_crop_alpha)))
            registration_closed=np.uint8(np.rint(other*closed_crop_alpha+neutral*(1-closed_crop_alpha)))
        # Rigidly register using hair, nose, cheeks and mouth; exclude both eyes.
        stable=np.full(self.original.shape[:2],255,np.uint8)
        for eye in self.specs:
            xs,ys=np.array(eye['poly']).T
            x0,x1=max(0,int(xs.min())-fx-3),min(fr-fx,int(xs.max())-fx+4)
            y0,y1=max(0,int(ys.min())-fy-3),min(fb-fy,int(ys.max())-fy+4)
            stable[y0:y1,x0:x1]=0
        warp=np.eye(2,3,dtype=np.float32)
        criteria=(cv2.TERM_CRITERIA_EPS|cv2.TERM_CRITERIA_COUNT,150,1e-6)
        self.ecc=None
        self.registration_warning=None
        if rig.get('registration','ecc')=='ecc':
            try:
                self.ecc,warp=cv2.findTransformECC(cv2.cvtColor(registration_open,cv2.COLOR_RGB2GRAY),cv2.cvtColor(registration_closed,cv2.COLOR_RGB2GRAY),warp,cv2.MOTION_EUCLIDEAN,criteria,stable,3)
            except cv2.error:
                self.registration_warning='ECC failed; identity alignment used. Inspect face contact sheet before accepting.'
                warp=np.eye(2,3,dtype=np.float32)
        elif rig.get('registration')!='none':
            raise ValueError('registration must be ecc or none')
        if closed_crop_alpha is None:
            self.registered=cv2.warpAffine(other,warp,self.size,flags=cv2.INTER_CUBIC|cv2.WARP_INVERSE_MAP,borderMode=cv2.BORDER_REFLECT)
        else:
            alpha=cv2.warpAffine(closed_crop_alpha,warp,self.size,flags=cv2.INTER_CUBIC|cv2.WARP_INVERSE_MAP,borderMode=cv2.BORDER_REFLECT)[...,None]
            premult=cv2.warpAffine(other.astype(np.float32)*closed_crop_alpha,warp,self.size,flags=cv2.INTER_CUBIC|cv2.WARP_INVERSE_MAP,borderMode=cv2.BORDER_REFLECT)
            color=np.divide(premult,np.maximum(alpha,1e-6))
            color=np.where(alpha>1e-6,color,self.original)
            self.registered=np.uint8(np.rint(np.clip(color,0,255)))
        self.open_hi=cv2.resize(self.original,None,fx=4,fy=4,interpolation=cv2.INTER_CUBIC).astype(np.float32)
        self.closed_hi=cv2.resize(self.registered,None,fx=4,fy=4,interpolation=cv2.INTER_CUBIC).astype(np.float32)
        self.hy,self.hx=np.mgrid[0:self.open_hi.shape[0],0:self.open_hi.shape[1]].astype(np.float32)
        self.worldx=self.hx[0]/4+fx
        self.worldy=self.hy/4+fy
        all_masks=np.zeros(self.open_hi.shape[:2],np.uint8)
        dark=cv2.cvtColor(np.uint8(self.closed_hi),cv2.COLOR_RGB2GRAY).astype(np.float32)
        for spec in self.specs:
            mask=np.zeros(all_masks.shape,np.uint8)
            poly=np.array([((x-fx)*4,(y-fy)*4) for x,y in spec['poly']],np.int32)
            cv2.fillPoly(mask,[poly],255)
            all_masks=np.maximum(all_masks,mask)
            spec['mask']=cv2.GaussianBlur(mask.astype(np.float32)/255,(9,9),1)[...,None]
            x0,x1=int(spec['x'][0])+1,int(spec['x'][-1])-1
            y0,y1=spec['closed_search']
            xs=np.arange(x0,x1+.1,.5)
            ys=[]
            for x in xs:
                column=round((x-fx)*4)
                ylow,yhigh=round((y0-fy)*4),round((y1-fy)*4)
                # The darkest feature in the closed eye is the real lash.
                strip=dark[ylow:yhigh,column-1:column+2].mean(axis=1)
                ys.append((ylow+int(np.argmin(strip)))/4+fy)
            coef=np.polyfit(xs,np.array(ys),2)
            spec['closed']=np.polyval(coef,self.worldx).astype(np.float32)
            spec['detected_lash_points']=list(zip(xs.tolist(),ys))
        # Remove just the real closed lashes from the closed reference. Inpaint
        # no iris-sized polygon and no hair below the eye: that produced a block.
        closed_lash_mask=np.zeros(all_masks.shape,np.uint8)
        for spec in self.specs:
            distance=abs(self.worldy-spec['closed'][None,:])
            horizontal=(self.worldx>=spec['x'][0]-1)&(self.worldx<=spec['x'][-1]+1)
            support=(distance<4.0)&horizontal[None,:]
            raw=((dark<closed_dark)&support).astype(np.uint8)*255
            raw=cv2.dilate(raw,np.ones((5,5),np.uint8))
            closed_lash_mask=np.maximum(closed_lash_mask,raw)
            spec['lash_support']=support
        self.skin=cv2.inpaint(np.uint8(self.closed_hi),closed_lash_mask,12,cv2.INPAINT_TELEA).astype(np.float32)
        for spec in self.specs:
            # A color matte carries the complete lash, including antialiased tip
            # pixels. Moving the matte never transports a rectangle of hair/skin.
            difference=(self.skin-self.closed_hi).mean(axis=2)
            alpha=np.clip(difference/100,0,1)*spec['lash_support']
            alpha=cv2.dilate(alpha,np.ones((3,3),np.uint8))
            spec['lash_alpha']=alpha.astype(np.float32)[...,None]
            spec['lash_premult']=np.clip(self.closed_hi-self.skin*(1-alpha[...,None]),0,255*alpha[...,None])
            # Use the actual open-eye lash near the end of reopening. Transporting
            # the wider closed lash all the way to open caused a visible shrink
            # when the exact original frame replaced it at the endpoint.
            upper=np.interp(self.worldx,spec['x'],spec['upper'])
            support=(abs(self.worldy-upper[None,:])<2.5)&((self.worldx>=spec['x'][0]-1)&(self.worldx<=spec['x'][-1]+1))[None,:]
            opened_gray=cv2.cvtColor(np.uint8(self.open_hi),cv2.COLOR_RGB2GRAY).astype(np.float32)
            red_lash=(self.open_hi[...,0]-self.open_hi[...,2])>red_margin
            raw=((opened_gray<open_dark)&red_lash&support).astype(np.uint8)*255
            raw=cv2.dilate(raw,np.ones((3,3),np.uint8))
            open_skin=cv2.inpaint(np.uint8(self.open_hi),raw,10,cv2.INPAINT_TELEA).astype(np.float32)
            delta=(open_skin-self.open_hi).mean(axis=2)
            open_alpha=np.clip(delta/85,0,1)*support*red_lash
            open_alpha=cv2.dilate(open_alpha.astype(np.float32),np.ones((3,3),np.uint8))
            spec['open_lash_alpha']=open_alpha[...,None]
            spec['open_lash_premult']=np.clip(self.open_hi-open_skin*(1-open_alpha[...,None]),0,255*open_alpha[...,None])
            # Transparent matte pixels must carry no RGB. Otherwise a transported
            # eyelash also adds a bright, unoccluded halo that pops off later.
            for prefix in ('lash','open_lash'):
                matte=spec[prefix+'_alpha'];color=spec[prefix+'_premult']
                assert np.all(color<=255*matte+1e-5)
                assert np.all(color[matte[...,0]<1e-7]==0)
        self.closed_native=self.original.astype(np.float32).copy()
        for spec in self.specs:
            native_mask=cv2.resize(spec['mask'],self.size,interpolation=cv2.INTER_AREA)[...,None]
            self.closed_native=self.closed_native*(1-native_mask)+self.registered.astype(np.float32)*native_mask
        self.closed_native=np.uint8(np.clip(self.closed_native,0,255))

    def aperture(self,spec,openness):
        source_upper=np.interp(self.worldx,spec['x'],spec['upper'])
        source_lower=np.interp(self.worldx,spec['x'],spec['lower'])
        closed=spec['closed'].astype(np.float64)
        upper=np.minimum(source_upper,closed)
        lower=np.maximum(source_lower,closed)
        top=closed+(upper-closed)*openness
        bottom=closed+(lower-closed)*openness
        horizontal=(self.worldx>=spec['x'][0]-.5)&(self.worldx<=spec['x'][-1]+.5)
        visible=np.clip((self.worldy-top[None,:])/.3,0,1)*np.clip((bottom[None,:]-self.worldy)/.3,0,1)*horizontal[None,:]
        return source_upper,closed,top,visible

    def render(self,openness):
        if openness>=.99999:
            return self.original.copy()
        # A fully covered iris has no information left to interpolate. Retain
        # exact native reference pixels throughout this final closure plateau.
        if openness<=.18:
            return self.closed_native.copy()
        fx,fy,fr,fb=self.box
        out=self.open_hi.copy()
        for spec in self.specs:
            mask=spec['mask']
            # At the outer corners the measured closed lash can sit below the
            # coarse open lower-lid guide. Interpolating toward that guide made
            # the aperture retreat and erase sclera that was already revealed.
            # Nested bounds preserve the original eye texture without scaling it.
            upper,closed,top,visible=self.aperture(spec,openness)
            eye=self.skin*(1-visible[...,None])+self.open_hi*visible[...,None]
            # Transport the real closed lash to the current upper lid. The iris
            # remains from a single unmorphed source and is clipped by both lids.
            mapy=(self.worldy+closed[None,:]-top[None,:]-fy)*4
            lash_alpha=np.clip(cv2.remap(spec['lash_alpha'],self.hx,mapy.astype(np.float32),cv2.INTER_LINEAR,borderMode=cv2.BORDER_CONSTANT),0,1)[...,None]
            lash=cv2.remap(spec['lash_premult'],self.hx,mapy.astype(np.float32),cv2.INTER_LINEAR,borderMode=cv2.BORDER_CONSTANT)
            open_mapy=(self.worldy+upper[None,:]-top[None,:]-fy)*4
            open_alpha=np.clip(cv2.remap(spec['open_lash_alpha'],self.hx,open_mapy.astype(np.float32),cv2.INTER_LINEAR,borderMode=cv2.BORDER_CONSTANT),0,1)[...,None]
            open_lash=cv2.remap(spec['open_lash_premult'],self.hx,open_mapy.astype(np.float32),cv2.INTER_LINEAR,borderMode=cv2.BORDER_CONSTANT)
            open_weight=float(np.clip((openness-.32)/.38,0,1))
            open_weight=open_weight*open_weight*(3-2*open_weight)
            combined_alpha=lash_alpha*(1-open_weight)+open_alpha*open_weight
            combined_lash=lash*(1-open_weight)+open_lash*open_weight
            eye=eye*(1-combined_alpha)+combined_lash
            out=out*(1-mask)+eye*mask
        result=cv2.resize(np.uint8(np.clip(out,0,255)),self.size,interpolation=cv2.INTER_AREA)
        # Converge smoothly before the final plateau, without resampling it.
        blend=float(np.clip((.34-openness)/.16,0,1))
        blend=blend*blend*(3-2*blend)
        result=result*(1-blend)+self.closed_native*blend
        # After switching to the matching original lash, settle the remaining
        # subpixel sampling/skin differences continuously. At this point lash
        # displacement is under one pixel, so no separated/double lash is mixed.
        settle=float(np.clip((openness-.92)/.08,0,1))
        settle=settle*settle*(3-2*settle)
        result=result*(1-settle)+self.original.astype(np.float32)*settle
        return np.uint8(np.rint(np.clip(result,0,255)))
