"""Transactional client profile layer, composed over existing cosmetic packages."""
from pathlib import Path
import hashlib,io,json,os,shutil,uuid
from PIL import Image,ImageOps
from modern_upk import Package
from fake_display import patch_package,validate_name
from menu_guard import require_closed
from swap_manager import sha,load_state,write_state,pristine,filename,is_epic_original,MODIFIED
from job_progress import report as progress

SCHEMA=1
CODEC_REVISION=10
def load(root):
    path=Path(root)/'data/profile.json'
    if not path.exists():return {'schema':SCHEMA,'settings':{'name':'','avatar':'','banner':''},'packages':{}}
    value=json.loads(path.read_text(encoding='utf8'))
    if value.get('schema')!=SCHEMA:raise ValueError('Versión de perfil no reconocida.')
    return value

def save(root,value):
    path=Path(root)/'data/profile.json';path.parent.mkdir(parents=True,exist_ok=True)
    tmp=path.with_suffix('.tmp');tmp.write_text(json.dumps(value,ensure_ascii=True,indent=2),encoding='utf8');os.replace(tmp,path)

def import_image(root,source,kind):
    if kind not in ('avatar','banner'):raise ValueError('Tipo de imagen no válido.')
    source=Path(source)
    if source.stat().st_size>20*1024*1024:raise ValueError('La imagen supera los 20 MB.')
    with Image.open(source) as image:
        if image.width*image.height>40_000_000:raise ValueError('Imagen demasiado grande.')
        image=ImageOps.exif_transpose(image).convert('RGBA')
        image=ImageOps.fit(image,(512,512) if kind=='avatar' else (840,200),method=Image.Resampling.LANCZOS)
        out=io.BytesIO();image.save(out,format='PNG');raw=out.getvalue()
    folder=Path(root)/'data/profile/images';folder.mkdir(parents=True,exist_ok=True)
    target=folder/(hashlib.sha256(raw).hexdigest()+'.png');target.write_bytes(raw)
    return str(target.resolve())

def normalize(root,settings):
    result={'name':validate_name(settings.get('name',''))}
    from title_display import normalize_title
    result['title']=normalize_title(settings.get('title'))
    from rank_display import normalize_rank
    result['rank']=normalize_rank(settings.get('rank'))
    image_root=(Path(root)/'data/profile/images').resolve()
    for key in ['avatar','banner']:
        value=settings.get(key,'')
        if value:
            p=Path(value).resolve()
            if p.parent!=image_root or not p.is_file() or p.suffix!='.png':raise ValueError('Importa la imagen desde el editor de perfil.')
            with Image.open(p) as im:im.verify()
            result[key]=str(p)
        else:result[key]=''
    return result

def _base(root,cooked,name,profile):
    entry=profile['packages'].get(name)
    if not entry:return cooked/name
    if sha(cooked/name)!=entry['installed_sha256']:
        if is_epic_original(cooked,name):return cooked/name  # reset by verify/update: rebuild on the original
        raise ValueError(MODIFIED.format(name))
    path=Path(entry['base'])
    if sha(path)!=entry['base_sha256']:raise ValueError('La base del perfil no coincide: '+name)
    return path

def _archive(root,source):
    digest=sha(source);folder=Path(root)/'data/profile/bases'/digest;folder.mkdir(parents=True,exist_ok=True)
    target=folder/source.name
    if not target.exists():shutil.copyfile(source,target)
    if sha(target)!=digest:raise ValueError('No se verificó la base del perfil.')
    return target,digest

SECTIONS=('name','title','rank','avatar','banner')

def _unaffected(cooked,settings,profile):
    """Profile packages that need no rebuild when only some sections changed; None means rebuild all.
    A section switched on for the first time is unknown territory, so it forces a full scan."""
    old=profile.get('settings',{});changed={k for k in SECTIONS if settings.get(k)!=old.get(k)}
    packages=profile.get('packages',{})
    if (not changed or profile.get('codec_revision')!=CODEC_REVISION or profile.get('partial') or not packages
            or any(not old.get(k) for k in changed-{'banner'}) or any('features' not in e for e in packages.values())):
        return None
    return {n for n,e in packages.items() if not set(e['features'])&changed and sha(cooked/n)==e['installed_sha256']}

def equipped_banner(api):
    """Package of the banner equipped on the account, from the local save."""
    from player_inventory import load_snapshot
    snap=load_snapshot(api.read_database(api.DEFAULT_ENGINE))
    return next((r.get('AssetPackage') for r in snap.account_equipped if r.get('Slot')=='Player Banner' and r.get('AssetPackage')),None)

def _generate(api,game,settings,profile,banner_package=None):
    cooked=Path(game)/'TAGame/CookedPCConsole'
    keep=_unaffected(cooked,settings,profile)
    if keep is not None and banner_package!=profile.get('banner_package'):keep-={profile.get('banner_package')}
    names=set(profile['packages'])
    if settings['name'] or settings.get('title') or settings['avatar'] or settings.get('rank'):
        names.update(p.name for p in cooked.glob('*.upk') if p.name.lower().startswith('gfx') and 'sounds' not in p.name.lower())
    if settings['avatar'] or settings['banner']:names.add('Startup.upk')
    # Startup contains the shared components/code loaded by the other menus.
    if settings['name'] or settings.get('title') or settings.get('rank'):names.add('Startup.upk')
    if keep is not None:names=set(profile['packages'])-keep
    if settings['banner']:
        names.add('Startup.upk')
        if banner_package and (cooked/banner_package).is_file():names.add(banner_package)
    images={k:Path(settings[k]).read_bytes() for k in ['avatar','banner'] if settings[k]}
    folder=api.ROOT/'paquetes'/('perfil-'+uuid.uuid4().hex);folder.mkdir(parents=True)
    jobs=[];new={'schema':SCHEMA,'codec_revision':CODEC_REVISION,'settings':settings,'banner_package':banner_package if settings['banner'] else None,'packages':{n:profile['packages'][n] for n in keep or ()}}
    from concurrent.futures import ThreadPoolExecutor,as_completed
    def prepare_one(name):
        require_closed();filename(name);original=pristine(cooked,name,api.ROOT/'data')
        source=_base(api.ROOT,cooked,name,profile)
        # Check current tracked state even when no previous profile layer exists.
        state=load_state(api.ROOT/'data').get(name)
        if state and sha(cooked/name) not in {state['original_sha256'],state['installed_sha256']}:
            raise ValueError(MODIFIED.format(name))
        # Archive by the original package name; never feed a renamed UPK to codecs.
        base,digest=_archive(api.ROOT,source)
        pkg=Package(source,api.DEFAULT_ENGINE)
        raw,validation=patch_package(pkg,settings['name'],images if name=='Startup.upk' else {'banner':images['banner']} if name==banner_package and 'banner' in images else None,settings.get('title'),images.get('avatar'),settings.get('rank'))
        if raw is None:
            if name not in profile['packages']:return None
            raw=source.read_bytes();validation={'only_client_presentation':True,'other_payloads_unchanged':True,'exports':[]}
        else:new['packages'][name]={'base':str(base.resolve()),'base_sha256':digest,'installed_sha256':hashlib.sha256(raw).hexdigest(),'exports':validation['exports'],'features':validation.get('features',[])}
        job=folder/name;(job/'generado').mkdir(parents=True);generated=job/'generado'/name;generated.write_bytes(raw)
        report={'schema':3,'kind':'profile','target':{'Product':'Perfil local','AssetPackage':name,'Slot':'Profile'},
                'donor':{'Product':settings['name'] or 'Avatar / banner personalizado','Slot':'Profile'},
                'original_sha256':{name:sha(original)},'generated_sha256':sha(generated),'validation':validation,
                'profile_base_state':state if source==cooked/name else profile['packages'][name].get('base_state',state)}
        if name in new['packages']:new['packages'][name]['base_state']=report['profile_base_state']
        (job/'informe.json').write_text(json.dumps(report,ensure_ascii=True,indent=2),encoding='utf8');return job
    with ThreadPoolExecutor(max_workers=4,thread_name_prefix='profile-package') as pool:
        futures=[pool.submit(prepare_one,name) for name in sorted(names)]
        for index,future in enumerate(as_completed(futures),1):
            job=future.result()
            if job is not None:jobs.append(job)
            progress('prepare',index,len(names),'Preparando perfil y título')
    jobs.sort(key=lambda p:p.name)
    return jobs,new

def prepare(api,game,settings):
    require_closed();profile=load(api.ROOT)
    if 'title' not in settings:settings={**settings,'title':profile['settings'].get('title')}
    if 'rank' not in settings:settings={**settings,'rank':profile['settings'].get('rank')}
    settings=normalize(api.ROOT,settings)
    if settings.get('title'):
        from player_inventory import load_snapshot
        snap=load_snapshot(api.read_database(api.DEFAULT_ENGINE))
        owned={r.get('title_id') for r in snap.inventory if not r.get('blueprint')}
        owned.update(r['Product'] for r in snap.account_equipped if r.get('Slot')=='Player Title')
        if settings['title']['target'] not in owned:raise ValueError('El título original no aparece en tu inventario guardado.')
        equipped=next((r['Product'] for r in snap.account_equipped if r.get('Slot')=='Player Title'),'')
        if settings['title']['target']!=equipped:raise ValueError('Equipa el título original elegido en Rocket League y ciérralo antes de aplicar.')
    banner_package=equipped_banner(api) if settings.get('banner') else None
    if profile.get('codec_revision')==CODEC_REVISION and not profile.get('partial') and settings==profile['settings'] and banner_package==profile.get('banner_package'):
        cooked=Path(game)/'TAGame/CookedPCConsole'
        if all(sha(cooked/name)==entry['installed_sha256'] for name,entry in profile['packages'].items()):
            progress('done',1,1,'El perfil ya estaba aplicado.')
            return [],profile
    return _generate(api,game,settings,profile,banner_package)

def commit_jobs(api,jobs,new):
    # Reconcile the generic package registry with its underlying cosmetic state.
    state=load_state(api.ROOT/'data')
    for job in jobs:
        report=json.loads((job/'informe.json').read_text(encoding='utf8'));name=report['target']['AssetPackage'];base=report['profile_base_state']
        if name in new['packages']:
            if base:
                state[name]={**base,'installed_sha256':report['generated_sha256'],'status':'installed','profile_layer':True}
            else:state[name]['profile_layer']=True
        elif base:state[name]=base
        else:state[name]['status']='restored';state[name].pop('profile_layer',None)
    write_state(api.ROOT/'data',state);save(api.ROOT,new)
    return {'count':len(jobs),'state':state,'profile':new}

def apply(api,game,settings):
    from change_queue import install_all
    jobs,new=prepare(api,game,settings)
    count=install_all(api,jobs,game)
    result=commit_jobs(api,jobs,new);result['count']=count;result['cached']=not jobs
    from generated_cache import discard
    discard(api.ROOT,jobs)
    return result


def compose_jobs(api,jobs,profile_override=None):
    """Put the profile over newly selected skins; commit metadata after install succeeds."""
    profile=load(api.ROOT) if profile_override is None else profile_override;updated=json.loads(json.dumps(profile))
    if not profile['packages']:return updated
    updated['_composed']=[]
    for job in jobs:
        report_file=job/'informe.json';report=json.loads(report_file.read_text(encoding='utf8'));name=report['target']['AssetPackage']
        if name not in profile['packages']:continue
        source=job/'generado'/name;base,digest=_archive(api.ROOT,source)
        settings=profile['settings'];all_images={k:Path(settings[k]).read_bytes() for k in ['avatar','banner'] if settings.get(k)}
        images=all_images if name=='Startup.upk' else {'banner':all_images['banner']} if name==profile.get('banner_package') and 'banner' in all_images else None
        raw,validation=patch_package(Package(source,api.DEFAULT_ENGINE),settings.get('name',''),images,settings.get('title'),all_images.get('avatar'),settings.get('rank'))
        if raw is None:raise ValueError('No se pudo conservar el perfil en '+name)
        source.write_bytes(raw);report['generated_sha256']=sha(source);report['profile_composition']=validation
        report_file.write_text(json.dumps(report,ensure_ascii=True,indent=2),encoding='utf8')
        updated['packages'][name]={'base':str(base.resolve()),'base_sha256':digest,'installed_sha256':sha(source),'exports':validation['exports'],'features':validation.get('features',[]),'base_state':None}
        updated['_composed'].append(name)
    return updated

def commit_composed(api,profile):
    state=load_state(api.ROOT/'data')
    for name in profile.pop('_composed',[]):
        entry=profile['packages'][name]
        if name in state:
            entry['base_state']={**state[name],'installed_sha256':entry['base_sha256']}
            if entry['base_sha256']==state[name].get('original_sha256'):
                entry['base_state']['status']='restored';state[name]['status']='restored'
            state[name]['profile_layer']=True
    save(api.ROOT,profile);write_state(api.ROOT/'data',state)

def after_restore(root,names):
    profile=load(root);selected=set(profile['packages']) if names is None else set(names)&set(profile['packages'])
    for name in selected:profile['packages'].pop(name,None)
    # A partial package restore cannot represent a complete profile installation.
    profile['partial']=bool(profile['packages']) and bool(selected)
    if not profile['packages']:profile['settings']={'name':'','avatar':'','banner':''};profile['partial']=False
    state=load_state(Path(root)/'data')
    for name in selected:
        if name in state:state[name].pop('profile_layer',None)
    write_state(Path(root)/'data',state);save(root,profile)
