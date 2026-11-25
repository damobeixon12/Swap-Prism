"""Prepare all plans before installing. One user action; closed-game writes only."""
from pathlib import Path
import json,shutil,tempfile
from menu_guard import require_closed
from swap_manager import apply_job,load_state,write_state,sha
import texture_engine

def latest_per_category(plans):
    result={}
    for plan in plans:
        category=category_of(plan)
        result[category]=plan
    return list(result.values())

def category_of(plan):
    return 'Workshop' if plan['kind'].startswith('workshop') else plan['target']['Slot'] if plan['kind']=='item' else plan['pack'].category

def superseded_jobs(api,plans,jobs,game):
    """Restore a previous source of the same category in this transaction."""
    import uuid
    categories={category_of(p) for p in plans}
    occupied={json.loads((j/'informe.json').read_text(encoding='utf8'))['target']['AssetPackage'] for j in jobs}
    from swap_manager import pristine
    extra=[]
    for name,entry in load_state(api.ROOT/'data').items():
        if name in occupied or entry.get('status')!='installed' or entry.get('hud_display_companion'):continue
        item=entry.get('target_item',{});category=item.get('Slot')
        if category=='Texture':category=item.get('Product')
        if category not in categories:continue
        require_closed();source=pristine(Path(game)/'TAGame/CookedPCConsole',name,api.ROOT/'data')
        job=api.ROOT/'paquetes'/('replace-'+uuid.uuid4().hex);(job/'generado').mkdir(parents=True)
        shutil.copyfile(source,job/'generado'/name)
        report={'kind':'restore','restore_selection':True,'target':item,'donor':{'Product':'Original','Slot':item.get('Slot')},'original_sha256':{name:sha(source)},'generated_sha256':sha(source),'validation':{'serialized_payloads_unchanged':True}}
        (job/'informe.json').write_text(json.dumps(report),encoding='utf8');extra.append(job)
    return extra

def prepare_all(api,plans,game,version,log=print):
    require_closed();plans=latest_per_category(plans);jobs=[];packages=set()
    for n,plan in enumerate(plans,1):
        require_closed();log(f'Preparando {n}/{len(plans)}: '+plan['label'])
        if plan['kind'].startswith('workshop'):
            import workshop
            job=workshop.prepare(api,game,plan)
        elif plan['kind']=='item':
            job=api.generate(api.DEFAULT_ENGINE,game,api.ROOT/'paquetes',plan['target']['ID'],plan['donor']['ID'],version,log,
                             car=plan['car'],compatibility_confirmed=True,color=plan.get('color','Original'))
        else:
            job=texture_engine.generate(api,game,api.DEFAULT_ENGINE,plan['pack'],api.ROOT/'paquetes',plan.get('target'),plan.get('boost_texture'))
        report=json.loads((job/'informe.json').read_text(encoding='utf-8'));name=report['target']['AssetPackage']
        if name.casefold() in packages:raise ValueError('Dos cambios usan el mismo paquete: '+name+'. Conserva uno en la cola.')
        packages.add(name.casefold());jobs.append(job)
        companions=(texture_engine.generate_meter_companions if plan['kind']=='texture' and plan['pack'].category=='Boostmeter' else
                    texture_engine.generate_ball_companions if plan['kind']=='texture' and plan['pack'].category=='Pelota' else None)
        if companions:
            for companion in companions(api,game,plan['pack'],api.ROOT/'paquetes'):
                companion_report=json.loads((companion/'informe.json').read_text(encoding='utf8'));companion_name=companion_report['target']['AssetPackage']
                if companion_name.casefold() in packages:raise ValueError('Dos cambios usan la biblioteca '+companion_name)
                packages.add(companion_name.casefold());jobs.append(companion)
    return jobs+superseded_jobs(api,plans,jobs,game)

def install_all(api,jobs,game,log=print):
    require_closed();folder=api.ROOT/'data';state=load_state(folder);cooked=Path(game)/'TAGame/CookedPCConsole'
    reports=[json.loads((job/'informe.json').read_text(encoding='utf-8')) for job in jobs]
    # Capture current tracked versions, so failure restores the previous selection, not a random default.
    with tempfile.TemporaryDirectory(prefix='localskins-transaction-') as tmp:
        saved=[]
        for report in reports:
            from swap_manager import filename
            name=filename(report['target']['AssetPackage']);target=cooked/name
            old=Path(tmp)/name;shutil.copyfile(target,old);saved.append((target,old,sha(target)))
        attempted=[]
        try:
            for job,report in zip(jobs,reports):
                attempted.append(report)
                log('Aplicando '+report['target']['Product']+'…');apply_job(job,cooked,folder)
        except Exception as exc:
            try:
                require_closed()
                current_state=load_state(folder)
                for report in attempted:
                    name=report['target']['AssetPackage']
                    target,old,digest=next(saved_entry for saved_entry in saved if saved_entry[0].name==name)
                    require_closed()
                    if sha(old)!=digest:raise ValueError('La copia de transacción cambió.')
                    current=sha(target)
                    if current not in {digest,report['generated_sha256'],report['original_sha256'][name]}:
                        raise ValueError('Otro programa cambió '+name+'; no se sobrescribe.')
                    if current!=digest:
                        shutil.copyfile(old,target)
                        if sha(target)!=digest:raise ValueError('No se verificó la reversión de '+name)
                    if name in state:current_state[name]=state[name]
                    else:current_state.pop(name,None)
                    write_state(folder,current_state)
                write_state(folder,state)
            except Exception as rollback:
                raise ValueError(f'Aplicación incompleta: {exc}. Restauración de transacción pendiente: {rollback}. Conserva data y usa Restaurar con el juego cerrado.') from exc
            raise ValueError('No se aplicó la cola; se recuperó el estado anterior. '+str(exc)) from exc
    return len(jobs)
