"""Generate a validated local cosmetic replacement; installation is explicit."""
from pathlib import Path
from collections import Counter
import json
import shutil
import uuid
from modern_upk import Package, rewire
from swap_manager import pristine
from menu_guard import require_editable
from cosmetic_colors import paint


def generate(api,engine,game,output,target_id,donor_id,version='',log=print,car='',compatibility_confirmed=False,color='Original'):
    require_editable()
    engine,game=Path(engine).resolve(),Path(game).resolve()
    output=api.separate_output(output,game,engine)
    rows=api.catalog(engine,game)
    targets=[row for row in rows if str(row['ID'])==str(target_id)]
    donors=[row for row in rows if str(row['ID'])==str(donor_id)]
    if len(targets)!=1 or len(donors)!=1:
        raise ValueError('Elige dos objetos del catálogo disponible.')
    target,donor=targets[0],donors[0]
    if target['ID']==donor['ID'] and color=='Original':
        raise ValueError('Elige una apariencia distinta.')
    if target['Slot']!=donor['Slot']:
        raise ValueError('Los dos objetos deben ser de la misma categoría.')
    for item in (target,donor):
        allowed,reason=api.compatibility(item,car)
        if not allowed:
            raise ValueError(reason)
    if not compatibility_confirmed:
        raise ValueError('Confirma que tu coche admite los cosméticos elegidos.')
    if target['Slot']=='Body':
        if color!='Original':raise ValueError('El puente de carrocería conserva la pintura original. Selecciona Original.')
        from body_visual import generate as generate_body
        return generate_body(api,engine,game,output,target,donor,version,log)
    uses=Counter(row.get('AssetPackage','').casefold() for row in api.read_database(engine))
    if any(uses[item['AssetPackage'].casefold()]>1 for item in (target,donor)):
        raise ValueError('El paquete es compartido y requiere soporte de edición por objeto.')
    cooked=game/'TAGame/CookedPCConsole'
    names=sorted({api.safe_package(item['AssetPackage']) for item in (target,donor)})
    originals_from={name:pristine(cooked,name,api.ROOT/'data') for name in names}
    live_before={name:api.digest(cooked/name) for name in names}
    job=output/uuid.uuid4().hex
    originals,staged=job/'originales',job/'generado'
    originals.mkdir(parents=True)
    staged.mkdir()
    original_hash={}
    for name,source in originals_from.items():
        source_hash=api.digest(source)
        shutil.copy2(source,originals/name)
        if api.digest(originals/name)!=source_hash or api.digest(source)!=source_hash:
            raise ValueError('Un archivo cambió durante la copia.')
        original_hash[name]=source_hash
    log('Reconstruyendo la referencia cosmética y validando los efectos…')
    try:
        donor_package=Package(originals/donor['AssetPackage'],engine)
        target_package=Package(originals/target['AssetPackage'],engine)
        color_report=None
        if color!='Original':
            donor_package,color_report=paint(donor_package,color,cooked)
        result,validation=rewire(donor_package,target_package,donor,target)
        validation['color']=color_report
        generated=staged/target['AssetPackage']
        generated.write_bytes(result)
        if live_before!={name:api.digest(cooked/name) for name in names}:
            raise ValueError('Los paquetes del juego cambiaron durante la generación.')
        report={'schema':2,'status':'generated','game_version':version,'target':target,'donor':donor,
                'car':car,'original_sha256':original_hash,'generated_sha256':api.digest(generated),
                'validation':validation,'online_verified':False,
                'training_tested_pair':target['ID']==5287 and donor['ID']==2044 and version=='++Prime+Update60.1-CL-533021',
                'note':'Cosmético local. Los assets cargados pueden requerir reiniciar el juego.'}
        (job/'informe.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
        (job/'motor.log').write_text(json.dumps(validation,ensure_ascii=False,indent=2),encoding='utf-8')
    except Exception as exc:
        (job/'error.log').write_text(str(exc),encoding='utf-8')
        raise
    log('Paquete preparado y validado. Se aplica con Rocket League cerrado.')
    return job
