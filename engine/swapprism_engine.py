"""Private JSON worker for the Rust application. Never starts the old Tk UI."""
import contextlib,json,os,sys,runpy,re
from pathlib import Path
from dataclasses import asdict
import local_skins as api
from player_inventory import load_snapshot
from swap_manager import load_state,restore_all,inspect_state
from texture_library import load_library,scan,save_library,image_for
from change_queue import prepare_all,install_all
from cosmetic_colors import PAINTS
import profile_manager
import studio_backend
import workshop
import media_store
from menu_guard import require_closed

def player_name(source):
    """Read only the primary login matching this save's account, never a team name."""
    source=Path(source)
    tagame=next((p for p in source.parents if p.name.casefold()=='tagame'),None)
    if tagame is None:return ''
    log=tagame/'Logs/Launch.log'
    try:
        with log.open('rb') as stream:
            stream.seek(max(0,log.stat().st_size-2*1024*1024));text=stream.read().decode('utf8',errors='replace')
    except OSError:return ''
    name=''
    for line in text.splitlines():
        match=re.search(r'HandleLocalPlayerLoginStatusChanged PlayerName=(.*?) PlayerID=([^\s]+).*LoginStatus=LS_LoggedIn.*IsPrimary=True',line)
        if not match:continue
        identity=match[2].split('|')
        if len(identity)<2 or not identity[1] or identity[1].casefold() not in source.stem.casefold():continue
        candidate=match[1].strip()
        if 0<len(candidate)<=96 and not any(ord(c)<32 for c in candidate):name=candidate
    return name

def packs():
    library=load_library(api.ROOT/'data/texture_library');preview=api.ROOT/'data/texture_previews';preview.mkdir(parents=True,exist_ok=True)
    result=[]
    for p in library:
        image=preview/(p.digest+'.png')
        if not image.exists():
            im=image_for(p);im.thumbnail((256,192));im.save(image)
        result.append({k:getattr(p,k) for k in ('name','category','body_id','skin_id','digest')}|{'preview':str(image)})
    return result

def request(obj):
    action=obj.get('action');rows=api.read_database(api.DEFAULT_ENGINE)
    if action=='media_catalog':return {'media':media_store.catalog(api,obj['source'],obj.get('query',''),obj.get('refresh',False),obj.get('url','')),'sounds':media_store.audio_library(api)}
    if action=='media_preview':return media_store.preview_sound(api,obj['key'],obj.get('kind','post'))
    if action=='media_download':
        result=media_store.download(api,obj['source'],obj['key'],obj.get('kind','post'),obj.get('preview',False))
        if obj['source']!='sounds':result['packs']=packs()
        return result
    if action=='workshop_catalog':return workshop.refresh(api) if obj.get('refresh') else workshop.snapshot(api)
    if action=='workshop_download':return workshop.download(api,obj['key'],obj.get('confirmed') is True)
    if action=='workshop_import':return workshop.import_map(api,obj['path'])
    if action=='workshop_remove':return workshop.remove(api,obj['key'])
    if action=='refresh_state':return {'state':inspect_state(api.ROOT/'data')}
    if action=='bootstrap':
        errors=[]
        try:
            snap=load_snapshot(rows);inventory=asdict(snap);inventory['source']=str(snap.source);inventory['active']=snap.active;inventory['player_name']=player_name(snap.source)
            selected=next((r['Product'] for r in snap.account_equipped if r.get('Slot')=='Player Title'),'')
            title_ids={r['title_id'] for r in snap.inventory if r.get('title_id') and not r.get('blueprint')}
            if selected:title_ids.add(selected)
            inventory['titles']=[{'id':key,'equipped':key==selected} for key in sorted(title_ids)]
        except (ValueError,OSError) as exc:inventory={'inventory':[],'active':None};errors.append(str(exc))
        return {'snapshot':inventory,'packs':packs(),'sounds':media_store.audio_library(api),'state':inspect_state(api.ROOT/'data'),'colors':list(PAINTS),'errors':errors,'profile':profile_manager.load(api.ROOT),'studio':studio_backend.load(api),'audio_limits':studio_backend.manifest(api)}
    if action=='sound_import':return studio_backend.import_mp3(api,obj['path'],obj['kind'])
    if action=='sound_preview':
        import winsound
        path=Path(obj['path']).resolve()
        if path.parent!=(api.ROOT/'data/sounds').resolve() or path.suffix!='.wav':raise ValueError('Preescucha no válida.')
        info=json.loads(path.with_suffix('.json').read_text(encoding='utf8'));limit=studio_backend.manifest(api)[info['kind']]['max_seconds']
        clip=studio_backend.clip_wave(path.read_bytes(),obj.get('start',0.),obj.get('end',min(info['seconds'],limit)),limit)
        winsound.PlaySound(studio_backend.with_volume(clip,obj.get('volume',1.0)),winsound.SND_MEMORY)
        return {'previewed':True}
    if action=='studio_apply':
        game=Path(obj['game']).resolve()
        if not (game/'TAGame/CookedPCConsole').is_dir() or not (game/'Binaries/Win64/RocketLeague.exe').is_file():raise ValueError('Selecciona la instalación de Rocket League.')
        return studio_backend.apply(api,game,obj['settings'])
    if action=='profile_import':
        return {'path':profile_manager.import_image(api.ROOT,obj['path'],obj['kind']),'kind':obj['kind']}
    if action=='profile_apply':
        game=Path(obj['game']).resolve()
        if not (game/'TAGame/CookedPCConsole').is_dir() or not (game/'Binaries/Win64/RocketLeague.exe').is_file():raise ValueError('Selecciona la instalación de Rocket League.')
        return profile_manager.apply(api,game,obj['settings'])
    if action=='import':
        found,errors=scan(Path(obj['path']));save_library(api.ROOT/'data/texture_library',found)
        return {'packs':packs(),'errors':errors}
    if action=='restore':
        require_closed()
        names=obj.get('names')
        if names is not None:
            from texture_engine import METER_COMPANIONS
            state=load_state(api.ROOT/'data')
            if 'GFX_Hud_SF.upk' in names:
                names=list(dict.fromkeys(list(names)+[n for n in METER_COMPANIONS if state.get(n,{}).get('hud_display_companion')]))
        count=restore_all(api.ROOT/'data',names=names)
        profile_manager.after_restore(api.ROOT,names)
        return {'count':count,'state':load_state(api.ROOT/'data'),'profile':profile_manager.load(api.ROOT),'studio':studio_backend.after_restore(api,names)}
    if action!='apply':raise ValueError('Acción no disponible.')
    game=Path(obj['game']).resolve()
    if not (game/'TAGame/CookedPCConsole').is_dir() or not (game/'Binaries/Win64/RocketLeague.exe').is_file():raise ValueError('Selecciona la carpeta rocketleague que contiene Binaries y TAGame.')
    snapshot=load_snapshot(rows);owned={r['ID'] for r in snapshot.inventory if not r.get('blueprint')}
    active=snapshot.active or {};team=obj.get('team','Blue')
    teams=active.get('teams',[]);team_index=0 if team=='Blue' else 1
    equipped=teams[team_index].get('items',[]) if len(teams)>team_index else []
    if isinstance(equipped,dict):equipped=list(equipped.values())
    car=next((r['Product'] for r in equipped if isinstance(r,dict) and r.get('Slot')=='Body'),'')
    # Presets use lowercase loadout keys in some save versions.
    if not car:
        loadout=active.get('blue' if team=='Blue' else 'orange',[])
        if isinstance(loadout,dict):loadout=loadout.get('items',[])
        car=next((r['Product'] for r in loadout if isinstance(r,dict) and r.get('Slot')=='Body'),'')
    by_id={r['ID']:r for r in rows};library={p.digest:p for p in load_library(api.ROOT/'data/texture_library')};plans=[]
    for selected in obj['plans']:
        if selected.get('kind','').startswith('workshop'):
            if selected['kind'] not in {'workshop','workshop_restore'}:raise ValueError('Cambio de mapa no válido.')
            plans.append(selected);continue
        target=by_id.get(selected.get('target_id'))
        if target and target['ID'] not in owned:raise ValueError('El objeto original ya no aparece en tu inventario.')
        if selected['kind']=='item':
            donor=by_id[selected['donor_id']]
            if not target:raise ValueError('Elige el objeto original.')
            if not car:raise ValueError('No se ha identificado el coche del preset guardado. Abre el juego, equipa tu coche y vuelve a cerrar.')
            plans.append({'kind':'item','target':target,'donor':donor,'car':car,'color':selected.get('color','Original'),'label':selected['label']})
        else:
            pack=library[selected['digest']]
            if pack.category=='Decal':
                template=by_id.get(pack.body_id,{}).get('Product') if pack.body_id else ('Fennec' if 'fenec' in pack.source.casefold() else car)
                if not target or target['Slot']!='Decal' or template!=car:raise ValueError('La plantilla de decal debe corresponder al coche guardado.')
            elif pack.category not in ('Boostmeter','Pelota'):raise ValueError('Tipo de textura no disponible.')
            plans.append({'kind':'texture','pack':pack,'target':target,'label':selected['label']})
    profile_settings=obj.get('profile_settings')
    if not plans and not isinstance(obj.get('studio_settings'),dict) and not isinstance(profile_settings,dict):raise ValueError('La cola está vacía.')
    jobs=prepare_all(api,plans,game,obj.get('version',''))
    studio_settings=obj.get('studio_settings')
    if isinstance(studio_settings,dict):jobs+=studio_backend.prepare_jobs(api,game,studio_settings)
    if isinstance(studio_settings,dict):
        names=[json.loads((Path(j)/'informe.json').read_text(encoding='utf8'))['target']['AssetPackage'] for j in jobs]
        if len(names)!=len(set(names)):raise ValueError('Dos cambios afectan al mismo paquete. Aplica uno cada vez.')
    profile_jobs=[];prepared_profile=None
    if isinstance(profile_settings,dict):profile_jobs,prepared_profile=profile_manager.prepare(api,game,profile_settings)
    profile=profile_manager.compose_jobs(api,jobs,prepared_profile) if prepared_profile is not None else profile_manager.compose_jobs(api,jobs)
    occupied={json.loads((Path(j)/'informe.json').read_text(encoding='utf8'))['target']['AssetPackage'] for j in jobs} if profile_jobs else set()
    profile_jobs=[j for j in profile_jobs if json.loads((Path(j)/'informe.json').read_text(encoding='utf8'))['target']['AssetPackage'] not in occupied]
    jobs=profile_jobs+jobs
    count=install_all(api,jobs,game)
    if prepared_profile is not None:profile_manager.commit_jobs(api,profile_jobs,profile)
    profile_manager.commit_composed(api,profile)
    if isinstance(studio_settings,dict):studio_backend.commit_settings(api,studio_settings)
    result={'count':count,'state':load_state(api.ROOT/'data'),'profile':profile}
    if isinstance(studio_settings,dict):result['studio']=studio_settings
    from generated_cache import discard
    discard(api.ROOT,jobs)
    return result

if __name__=='__main__':
    if len(sys.argv)>=4 and sys.argv[1]=='--run-worker':
        script,log,*args=sys.argv[2:]
        with open(log,'w',encoding='utf8') as stream,contextlib.redirect_stdout(stream),contextlib.redirect_stderr(stream):
            sys.argv=[script,*args];runpy.run_path(script,run_name='__main__')
    else:
        try:
            obj=json.loads(sys.stdin.readline())
            with contextlib.redirect_stdout(sys.stderr):result=request(obj)
            # Frozen Python may use the Windows ANSI code page despite the parent's
            # UTF-8 environment. An ASCII JSON protocol preserves every Unicode name.
            print(json.dumps({'ok':True,'result':result},ensure_ascii=True,default=str))
        except Exception as exc:print(json.dumps({'ok':False,'error':str(exc)},ensure_ascii=True));sys.exit(1)
