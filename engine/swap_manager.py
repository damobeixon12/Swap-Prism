"""Install/restore validated cosmetic packages, with persistent verified backups."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import threading
import epic_manifest
from menu_guard import require_closed as require_editable


def sha(path):
    digest=hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda:stream.read(1024*1024),b''):
            digest.update(block)
    return digest.hexdigest()


def load_state(folder):
    path=Path(folder)/'swaps.json'
    return json.loads(path.read_text(encoding='utf-8')) if path.exists() else {}


def write_state(folder,state):
    folder=Path(folder)
    folder.mkdir(parents=True,exist_ok=True)
    temp=folder/'swaps.json.tmp'
    temp.write_text(json.dumps(state,ensure_ascii=False,indent=2),encoding='utf-8')
    os.replace(temp,folder/'swaps.json')


def inspect_state(folder):
    """Read current file hashes without rewriting backup/install metadata."""
    state=load_state(folder)
    profile_path=Path(folder)/'profile.json'
    profile=json.loads(profile_path.read_text(encoding='utf8')).get('packages',{}) if profile_path.exists() else {}
    for name,entry in state.items():
        try:
            current=sha(entry['target'])
            entry['observed_status']=('original' if current==entry.get('original_sha256') else
                                      'installed' if current==entry.get('installed_sha256') else 'external')
            layer=profile.get(name,{})
            if entry['observed_status']=='installed' and current==layer.get('installed_sha256') and layer.get('base_sha256')==entry.get('original_sha256'):
                entry['observed_status']='original'
            if entry['observed_status']=='external' and is_epic_original(Path(entry['target']).parent,name):
                entry['observed_status']='original'
        except (OSError,KeyError):entry['observed_status']='missing'
    return state


def verify_files(game,folder):
    """Emergency check: heal records of packages the launcher already reset, list packages changed by others.
    Never writes game files and never restores anything the user changed."""
    game=Path(game);cooked=game/'TAGame/CookedPCConsole'
    if epic_manifest.find(game) is None:
        return {'epic':False,'healed':[],'modified':[],'state':inspect_state(folder)}
    state=load_state(folder);healed=[]
    for name,entry in state.items():
        if entry.get('status')=='installed' and is_epic_original(cooked,name):
            rebase(cooked,name,folder);healed.append(name)
    tracked={name.casefold():entry for name,entry in load_state(folder).items()}
    modified=[]
    for path in sorted(cooked.glob('*')):
        if path.suffix.lower() not in {'.upk','.bnk'}:continue
        if is_epic_original(cooked,path.name) is not False:continue
        entry=tracked.get(path.name.casefold())
        if entry and entry.get('status')=='installed' and sha(path)==entry.get('installed_sha256'):continue  # our own change
        modified.append(path.name)
    return {'epic':True,'healed':healed,'modified':modified,'state':inspect_state(folder)}


def filename(name):
    if not isinstance(name,str) or '/' in name or '\\' in name or ':' in name or not (name.lower().endswith('.upk') or name in {'SFX_Car_Movements.bnk','SFX_Impacts.bnk'}):
        raise ValueError('Paquete no válido.')
    return name


MODIFIED=('{} está modificado por otro programa o por una versión anterior. '
          'Usa «Verificar archivos» (Epic → Biblioteca → Rocket League → ⋯ → Gestionar → Verificar) y vuelve a aplicar.')
_state_lock=threading.Lock()


def is_epic_original(cooked,name):
    """True/False against Epic's manifest; None when there is no manifest (Steam)."""
    want=epic_manifest.expected(Path(cooked).parent.parent,name)
    target=Path(cooked)/name
    return None if want is None or not target.exists() else epic_manifest.sha1(target)==want


def rebase(cooked,name,folder):
    """The game file is Epic's original again (verify/update): make it the backup and forget the old install."""
    with _state_lock:
        state=load_state(folder);entry=state.get(name)
        if not entry:return
        target=Path(cooked)/name;digest=sha(target);backup=Path(entry['backup'])
        if backup.exists() and sha(backup)==digest and entry.get('original_sha256')==digest and entry.get('status')!='installed':return
        backup.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(target,backup)
        if sha(backup)!=digest:raise ValueError('No se pudo renovar la copia original de '+name)
        entry['original_sha256']=digest;entry['status']='restored';entry['reload_status']='reset-by-launcher'
        entry.pop('profile_layer',None)
        write_state(folder,state)


def pristine(cooked,name,folder):
    name=filename(name)
    target=(Path(cooked)/name).resolve()
    entry=load_state(folder).get(name)
    if entry and target!=Path(entry['target']).resolve():
        raise ValueError('La instalación no coincide con la copia de seguridad.')
    original=is_epic_original(cooked,name)
    if original:
        rebase(cooked,name,folder)
        return target
    if original is False and not entry:
        raise ValueError(MODIFIED.format(name))
    if not entry:
        return target
    if target!=Path(entry['target']).resolve():
        raise ValueError('La instalación no coincide con la copia de seguridad.')
    backup=Path(entry['backup'])
    if sha(backup)!=entry['original_sha256']:
        raise ValueError('La copia original ha cambiado. No se usa.')
    if original is False and epic_manifest.sha1(backup)!=epic_manifest.expected(Path(cooked).parent.parent,name):
        raise ValueError(MODIFIED.format(name))
    if sha(target) not in {entry['original_sha256'],entry['installed_sha256']}:
        raise ValueError(MODIFIED.format(name))
    return backup


def apply_job(job,cooked,folder):
    gate=require_editable()
    job,cooked,folder=Path(job),Path(cooked).resolve(),Path(folder)
    report=json.loads((job/'informe.json').read_text(encoding='utf-8'))
    name=filename(report['target']['AssetPackage'])
    validation=report.get('validation',{})
    hud=validation.get('hud_display_color') or {}
    verified_hud=(hud.get('only_display_color_references_changed') is True or
                  (name.casefold() in {'gfx_hud_sf.upk','gfxshell_sf.upk','gfx_replayhud_sf.upk','gfx_spectatorhud_sf.upk'} and hud.get('only_meter_display_methods_changed') is True and
                   hud.get('label_and_number_textColor_explicit') is True and
                   set(hud.get('methods',[]))=={'onTeamColorChanged','onBoostChanged','updateBoost'}))
    if not (validation.get('serialized_payloads_unchanged') is True or
            (report.get('kind')=='texture' and validation.get('other_payloads_unchanged') is True and
             (validation.get('texture_and_material_bindings_only') is True or
              (validation.get('texture_and_hud_display_only') is True and verified_hud and
               validation.get('hud_display_color',{}).get('boost_amount_calculation_instructions_unchanged') is True) or
              (name=='GFX_StartMenu_SF.upk' and validation.get('only_start_menu_logo_presentation') is True))) or
            (report.get('kind')=='body_visual' and validation.get('original_body_physics_data_unchanged') is True and validation.get('only_visual_body_refs_changed') is True) or
            (report.get('kind')=='body_visual' and validation.get('original_body_physics_fields_unchanged') is True and validation.get('self_contained_visual_dependencies') is True) or
            (report.get('kind')=='profile' and validation.get('only_client_presentation') is True and validation.get('other_payloads_unchanged') is True) or
            (report.get('kind')=='studio' and name=='TAGame.upk' and validation.get('only_presentation_color_values') is True and validation.get('physics_and_functions_unchanged') is True) or
            (report.get('kind')=='sound' and name in {'SFX_Car_Movements.bnk','SFX_Impacts.bnk'} and validation.get('only_verified_audio_media') is True) or
            (report.get('kind')=='workshop' and name=='Labs_Utopia_P.upk' and validation.get('workshop_map_validated') is True and validation.get('fixed_utopia_retro_only') is True)):
        raise ValueError('El paquete no tiene la validación requerida.')
    staged=job/'generado'/name
    target=(cooked/name).resolve()
    if target.parent!=cooked:
        raise ValueError('Destino fuera de la carpeta del juego.')
    if sha(staged)!=report['generated_sha256']:
        raise ValueError('El paquete generado ha cambiado.')
    source=pristine(cooked,name,folder)
    state=load_state(folder)
    existing=state.get(name)
    original_sha=report['original_sha256'][name]
    if sha(source)!=original_sha:
        raise ValueError('El original ya no coincide con el usado al generar.')
    folder.mkdir(parents=True,exist_ok=True)
    backup=folder/'backups'/name
    backup.parent.mkdir(exist_ok=True)
    if not existing:
        if backup.exists() and sha(backup)!=original_sha:
            # A leftover copy from an older game version; replace it only with a verified original.
            if not is_epic_original(cooked,name):
                raise ValueError('Ya existe otra copia original distinta; no se sobrescribe.')
            backup.unlink()
        if not backup.exists():
            shutil.copy2(target,backup)
        if sha(backup)!=original_sha:
            raise ValueError('La copia de seguridad no coincide.')
    entry={'target':str(target),'backup':str(backup.resolve()),'original_sha256':original_sha,
           'installed_sha256':report['generated_sha256'],'target_item':report['target'],
           'donor_item':report['donor'],'game_version':report.get('game_version'),'status':'pending'}
    entry['color']=report.get('validation',{}).get('color')
    if report.get('hud_display_companion') is True:entry['hud_display_companion']=True
    state[name]=entry
    entry['reload_status']='not-running' if gate.mode=='closed' else 'pending-garage-refresh'
    write_state(folder,state)
    copying=False
    try:
        require_editable()
        copying=True
        shutil.copyfile(staged,target)
        if sha(target)!=report['generated_sha256']:
            raise ValueError('La copia instalada no coincide.')
        entry['status']='restored' if report.get('restore_selection') else 'installed'
        write_state(folder,state)
    except Exception:
        # Roll back only after confirming we still possess a valid original.
        if copying and sha(backup)==original_sha:
            require_editable()
            shutil.copyfile(backup,target)
            entry['status']='restored-after-error'
            write_state(folder,state)
        raise
    return entry


def restore_all(folder,names=None):
    gate=require_editable()
    state=load_state(folder)
    selected=None if names is None else {filename(name) for name in names}
    if selected is not None and not selected.issubset(state):
        raise ValueError('El cambio seleccionado no tiene copia registrada.')
    count=0
    for name,entry in state.items():
        if selected is not None and name not in selected:continue
        target,backup=Path(entry['target']),Path(entry['backup'])
        if sha(backup)!=entry['original_sha256']:
            raise ValueError('Copia de seguridad no válida: '+filename(name))
        current=sha(target)
        if current==entry['original_sha256']:
            entry['status']='restored'
            entry['reload_status']='not-running' if gate.mode=='closed' else 'reload-unconfirmed'
            continue
        if current!=entry['installed_sha256']:
            if is_epic_original(target.parent,name):
                # Already reset by the launcher: nothing to restore.
                entry['status']='restored';entry['reload_status']='reset-by-launcher'
                continue
            raise ValueError(MODIFIED.format(name))
        require_editable()
        shutil.copyfile(backup,target)
        if sha(target)!=entry['original_sha256']:
            raise ValueError('No se pudo verificar la restauración: '+name)
        entry['status']='restored'
        entry['reload_status']='not-running' if gate.mode=='closed' else 'pending-garage-refresh'
        count+=1
        write_state(folder,state)
    write_state(folder,state)
    return count
