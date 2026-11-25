"""Local presentation jobs and bounded MP3 import. No runtime game hooks."""
from pathlib import Path
import json,struct,math,uuid,hashlib,io,wave,os
from modern_upk import Package,i32,i64
from cosmetic_colors import export_properties,repack,properties
from swap_manager import pristine,sha,load_state
from menu_guard import require_closed

def manifest(api):
    path=(api.RESOURCES/'audio_manifest.json') if api.FROZEN else api.ROOT.parent.parent/'app/assets/audio_manifest.json'
    return json.loads(path.read_text(encoding='utf8'))

def palette_ranges(pkg):
    flat=pkg.flat();result={}
    for key,path,count in [('accent','CarColors.Accent',105),('blue','CarColors.BlueTeamV3',70),('orange','CarColors.OrangeTeamV3',70)]:
        index=pkg.find_asset(path);exp=pkg.exports[index]
        if pkg.class_name(exp)!='CarColorSet_TA':raise ValueError('Paleta no reconocida.')
        data,props=export_properties(pkg,flat,exp)
        field=next((v for v in props if v['name']=='Colors' and v['kind']=='ArrayProperty'),None)
        if not field or field['size']!=4+count*16 or i32(data,field['value'])!=count:raise ValueError('La versión del juego tiene otra paleta.')
        result[key]=(i64(exp,36)+field['value']+4,count)
    return result

def reset_ranges(pkg):
    flat=pkg.flat();result=[]
    for index,exp in enumerate(pkg.exports):
        if pkg.class_name(exp) not in ['ParticleModuleColor','ParticleModuleColorScaleOverLife']:continue
        if not pkg.path(index).startswith('FX_FlipReset.'):continue
        data,props=export_properties(pkg,flat,exp)
        for p in props:
            if p['name'] not in ['StartColor','ColorScaleOverLife'] or p['struct']!='RawDistributionVector':continue
            arr=data[p['value']:p['value']+p['size']];nested=properties(pkg,arr,0)
            if not nested:continue
            table=next((v for v in nested[1] if v['name']=='LookupTable' and v['kind']=='ArrayProperty'),None)
            if not table or table['size']!=36 or i32(arr,table['value'])!=8:raise ValueError('Tabla de color del flip reset desconocida.')
            # RawDistributionVector: first two floats are time range, then two RGB samples.
            start=i64(exp,36)+p['value']+table['value']+4+8
            result.extend([start,start+12])
    if not result:raise ValueError('No se localiza el color nativo del flip reset.')
    return result

def rgb(value):
    if not isinstance(value,str) or len(value)!=7 or value[0]!='#':raise ValueError('Color RGB no válido.')
    try:return tuple(int(value[i:i+2],16)/255 for i in [1,3,5])
    except ValueError:raise ValueError('Color RGB no válido.')

def patch_appearance(pkg,settings):
    original=pkg.flat();flat=bytearray(original);allowed=set();ranges=palette_ranges(pkg);changed=[]
    palette=settings.get('palette',{})
    if palette.get('enabled'):
        # Full selectable palette: preserve the game grid dimensions and slot indices.
        for key,(start,count) in ranges.items():
            values=palette.get(key,[])
            if len(values)!=count:raise ValueError('La paleta '+key+' requiere '+str(count)+' colores.')
            for n,value in enumerate(values):
                pos=start+n*16;flat[pos:pos+12]=struct.pack('<3f',*rgb(value));allowed.update(range(pos,pos+12))
        changed.append('Rich color palette')
    if settings.get('reset_enabled'):
        values=rgb(settings.get('reset_color'));starts=reset_ranges(pkg)
        for start in starts:
            flat[start:start+12]=struct.pack('<3f',*values);allowed.update(range(start,start+12))
        for start in starts[::2]:
            flat[start-8:start]=struct.pack('<ff',min(values),max(values));allowed.update(range(start-8,start))
        changed.append('Color del flip reset')
    cursor=0
    for pos in sorted(allowed):
        if pos>cursor and original[cursor:pos]!=flat[cursor:pos]:raise ValueError('Cambio fuera de las capas de color.')
        cursor=pos+1
    if original[cursor:]!=flat[cursor:]:raise ValueError('Cambio fuera de las capas de color.')
    return repack(pkg,bytes(flat)),{'only_presentation_color_values':True,'allowed_color_bytes':len(allowed),'features':changed,'physics_and_functions_unchanged':True}

def bank_chunks(raw):
    pos=0;chunks=[]
    while pos<len(raw):
        if pos+8>len(raw):raise ValueError('Banco de audio truncado.')
        tag=raw[pos:pos+4];size=struct.unpack_from('<I',raw,pos+4)[0]
        if pos+8+size>len(raw):raise ValueError('Banco de audio truncado.')
        chunks.append((tag,raw[pos+8:pos+8+size]));pos+=8+size
    if len({t for t,_ in chunks})!=len(chunks):raise ValueError('Secciones de audio repetidas.')
    return chunks

def patch_bank(raw,spec,pcm):
    if spec.get('disabled_reason'):raise ValueError(spec['disabled_reason'])
    if spec.get('isolation'):
        from post_isolation import isolate
        return isolate(raw,spec,pcm,bank_chunks)
    if hashlib.sha256(raw).hexdigest()!=spec['original_sha256']:raise ValueError('El banco de audio cambió con una actualización; requiere volver a identificarlo.')
    chunks=bank_chunks(raw);parts=dict(chunks);ids={m['id'] for m in spec['media']};hirc=bytearray(parts[b'HIRC']);new_data=bytearray();new_index=bytearray()
    for mid,offset,size in struct.iter_unpack('<III',parts[b'DIDX']):
        while len(new_data)%16:new_data.append(0)
        media=pcm if mid in ids else parts[b'DATA'][offset:offset+size]
        new_index.extend(struct.pack('<III',mid,len(new_data),len(media)));new_data.extend(media)
    for media in spec['media']:
        params=media['params']
        if any(p['value']!=0 for p in params['StreamType']):raise ValueError('El sonido no es un recurso incorporado.')
        for name,value in [('ulPluginID',0x10001),('uInMemoryMediaSize',len(pcm))]:
            for prop in params[name]:
                if struct.unpack_from('<I',hirc,prop['offset'])[0]!=prop['value']:raise ValueError('Metadatos de audio incompatibles.')
                struct.pack_into('<I',hirc,prop['offset'],value)
    updated={b'DIDX':bytes(new_index),b'DATA':bytes(new_data),b'HIRC':bytes(hirc)}
    output=b''.join(tag+struct.pack('<I',len(updated.get(tag,data)))+updated.get(tag,data) for tag,data in chunks)
    # Every unselected media payload and every unrelated section must remain exact.
    after=dict(bank_chunks(output));new={mid:after[b'DATA'][off:off+size] for mid,off,size in struct.iter_unpack('<III',after[b'DIDX'])}
    for mid,off,size in struct.iter_unpack('<III',parts[b'DIDX']):
        if mid not in ids and new[mid]!=parts[b'DATA'][off:off+size]:raise ValueError('Cambió un sonido no seleccionado.')
    return output

def import_mp3(api,path,kind):
    import miniaudio
    spec=manifest(api).get(kind)
    if not spec or not spec['media']:raise ValueError('Evento de audio no identificado.')
    path=Path(path)
    if path.suffix.lower()!='.mp3' or not path.is_file():raise ValueError('Selecciona un archivo MP3.')
    if path.stat().st_size>8*1024*1024:raise ValueError('El MP3 supera 8 MB.')
    info=miniaudio.get_file_info(str(path))
    if info.file_format!=miniaudio.FileFormat.MP3:raise ValueError('El contenido no es un MP3 válido.')
    if not math.isfinite(info.duration) or info.duration<=0 or info.duration>300:
        raise ValueError('Selecciona un MP3 de hasta 5 minutos para recortarlo.')
    sound=miniaudio.decode_file(str(path),output_format=miniaudio.SampleFormat.SIGNED16,nchannels=1,sample_rate=48000)
    samples=sound.samples
    if not samples or len(samples)>300*48000:raise ValueError('El audio supera 5 minutos.')
    output=io.BytesIO()
    with wave.open(output,'wb') as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(48000);w.writeframes(samples.tobytes())
    raw=output.getvalue();digest=hashlib.sha256(raw).hexdigest();folder=api.ROOT/'data/sounds';folder.mkdir(parents=True,exist_ok=True)
    wav=folder/(digest+'-'+kind+'.wav');wav.write_bytes(raw)
    peaks=[]
    for n in range(96):
        segment=samples[len(samples)*n//96:len(samples)*(n+1)//96];peaks.append(max((abs(v) for v in segment),default=0)/32768)
    duration=len(samples)/48000
    result={'kind':kind,'name':path.stem[:64],'path':str(wav),'sha256':digest,'seconds':duration,'peaks':peaks,'volume':1.0,'start':0.,'end':min(duration,spec['max_seconds'])}
    wav.with_suffix('.json').write_text(json.dumps(result),encoding='utf8')
    return result

def clip_wave(raw,start,end,limit):
    if not all(isinstance(v,(int,float)) and not isinstance(v,bool) and math.isfinite(v) for v in [start,end,limit]):raise ValueError('Recorte no válido.')
    with wave.open(io.BytesIO(raw),'rb') as w:
        if w.getnchannels()!=1 or w.getsampwidth()!=2 or w.getframerate()!=48000:raise ValueError('Audio no válido.')
        frames=w.getnframes();a=round(start*48000);b=round(end*48000)
        if a<0 or b<=a or b>frames+2 or b-a>round(limit*48000)+1:raise ValueError('El recorte supera la duración permitida.')
        b=min(b,frames)
        w.setpos(a);pcm=w.readframes(b-a)
    out=io.BytesIO()
    with wave.open(out,'wb') as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(48000);w.writeframes(pcm)
    return out.getvalue()

def with_volume(raw,volume):
    import array
    if isinstance(volume,bool) or not isinstance(volume,(float,int)) or not math.isfinite(volume) or not 0<=volume<=1:raise ValueError('Volumen no válido.')
    with wave.open(io.BytesIO(raw),'rb') as w:
        if w.getnchannels()!=1 or w.getsampwidth()!=2 or w.getframerate()!=48000:raise ValueError('Audio preparado no válido.')
        samples=array.array('h',w.readframes(w.getnframes()))
    if volume==1:return raw
    samples=array.array('h',(round(v*volume) for v in samples));stream=io.BytesIO()
    with wave.open(stream,'wb') as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(48000);w.writeframes(samples.tobytes())
    return stream.getvalue()

def wem_pcm(raw):
    """Wwise PCM with explicit channel layout and aligned sample data."""
    with wave.open(io.BytesIO(raw),'rb') as w:
        if (w.getnchannels(),w.getsampwidth(),w.getframerate())!=(1,2,48000):raise ValueError('Formato PCM preparado no válido.')
        pcm=w.readframes(w.getnframes())
    guid=bytes.fromhex('0100000000001000800000aa00389b71')
    fmt=struct.pack('<HHIIHHHHI16s',0xfffe,1,48000,96000,2,16,22,16,4,guid)
    def chunk(tag,data):return tag+struct.pack('<I',len(data))+data
    payload=b'WAVE'+chunk(b'fmt ',fmt)+chunk(b'JUNK',b'\0'*4)+chunk(b'data',pcm)
    result=b'RIFF'+struct.pack('<I',len(payload))+payload
    if result.index(b'data')+8!=80:raise ValueError('Alineación PCM incorrecta.')
    return result

def load(api):
    path=api.ROOT/'data/studio.json'
    return json.loads(path.read_text(encoding='utf8')) if path.exists() else {}

def prepare_jobs(api,game,settings):
    from change_queue import install_all
    from profile_manager import compose_jobs,commit_composed
    from job_progress import report
    report('studio',0,5,'Validando personalización')
    require_closed();cooked=Path(game)/'TAGame/CookedPCConsole';jobs=[];specs=manifest(api)
    old=load(api);state=load_state(api.ROOT/'data')
    for kind in ['reset','post']:
        if settings.get('sounds',{}).get(kind) and specs[kind].get('disabled_reason'):
            raise ValueError(specs[kind]['disabled_reason'])
    appearance=settings.get('palette',{}).get('enabled') or settings.get('reset_enabled')
    if appearance or old.get('palette',{}).get('enabled') or old.get('reset_enabled'):
        report('studio',1,5,'Preparando paleta y efecto de flip reset')
        name='TAGame.upk';source=pristine(cooked,name,api.ROOT/'data');pkg=Package(source,api.DEFAULT_ENGINE)
        result,validation=patch_appearance(pkg,settings)
        jobs.append(make_job(api,name,result.raw,source,'Colores y flip reset',validation,'studio'))
    for kind in ['reset','post']:
        chosen=settings.get('sounds',{}).get(kind);was=old.get('sounds',{}).get(kind)
        if not chosen and not was:continue
        spec=specs[kind];name=spec['bank'];source=pristine(cooked,name,api.ROOT/'data');raw=source.read_bytes()
        report('studio',2,5,'Preparando sonido '+kind)
        if chosen:
            folder=(api.ROOT/'data/sounds').resolve();path=Path(chosen['path']).resolve()
            if path.parent!=folder or sha(path)!=chosen['sha256']:raise ValueError('El sonido preparado ha cambiado. Impórtalo otra vez.')
            info=json.loads(path.with_suffix('.json').read_text(encoding='utf8'))
            if info['kind']!=kind:raise ValueError('Evento de audio no válido.')
            clip=clip_wave(path.read_bytes(),chosen.get('start',0.),chosen.get('end',min(info['seconds'],spec['max_seconds'])),spec['max_seconds'])
            raw=patch_bank(raw,spec,wem_pcm(with_volume(clip,chosen.get('volume',1.0))))
        jobs.append(make_job(api,name,raw,source,'Sonido '+('flip reset' if kind=='reset' else 'poste/travesaño'),{'only_verified_audio_media':True,'event':kind},'sound'))
    if settings.get('season_enabled') or old.get('season_enabled'):
        report('studio',3,5,'Preparando pantalla de temporada')
        name='GFX_StartMenu_SF.upk';source=pristine(cooked,name,api.ROOT/'data');pkg=Package(source,api.DEFAULT_ENGINE)
        raw=season_logo(api,pkg,settings.get('season',24)) if settings.get('season_enabled') else pkg.raw
        jobs.append(make_job(api,name,raw,source,'Inicio de temporada Swap Prism',{'other_payloads_unchanged':True,'only_start_menu_logo_presentation':True},'texture'))
    return jobs

def commit_settings(api,settings):
    path=api.ROOT/'data/studio.json';path.parent.mkdir(parents=True,exist_ok=True)
    path.with_suffix('.tmp').write_text(json.dumps(settings,indent=2),encoding='utf8');os.replace(path.with_suffix('.tmp'),path)

def apply(api,game,settings):
    from change_queue import install_all
    from profile_manager import compose_jobs,commit_composed
    from job_progress import report
    jobs=prepare_jobs(api,game,settings)
    profile=compose_jobs(api,jobs) if jobs else None
    report('studio',4,5,'Instalando cambios verificados')
    count=install_all(api,jobs,game) if jobs else 0
    if profile:commit_composed(api,profile)
    commit_settings(api,settings)
    report('studio',5,5,'Personalización aplicada')
    return {'count':count,'state':load_state(api.ROOT/'data'),'studio':settings,'profile':profile or {}}


def after_restore(api,names):
    settings=load(api)
    if not settings:return settings
    if names is None:settings={}
    else:
        if 'TAGame.upk' in names:
            settings['reset_enabled']=False
            if isinstance(settings.get('palette'),dict):settings['palette']['enabled']=False
        if 'GFX_StartMenu_SF.upk' in names:settings['season_enabled']=False
        for kind,spec in manifest(api).items():
            if spec['bank'] in names:settings.get('sounds',{}).pop(kind,None)
    path=api.ROOT/'data/studio.json';path.write_text(json.dumps(settings,indent=2),encoding='utf8');return settings

def season_logo(api,pkg,season):
    from PIL import Image,ImageDraw,ImageFont
    from texture_engine import inline_serial,append_exports
    if isinstance(season,bool) or not isinstance(season,int) or not 1<=season<=999:raise ValueError('Temporada del diseño no válida.')
    image=Image.new('RGBA',(1024,512));draw=ImageDraw.Draw(image)
    font=ImageFont.truetype('C:/Windows/Fonts/segoeuib.ttf',72);small=ImageFont.truetype('C:/Windows/Fonts/segoeuib.ttf',31)
    # Keep the supplied logo intact; remove only its black matte in this composition.
    logo_path=(api.RESOURCES/'logo.png') if api.FROZEN else api.ROOT.parent.parent/'app/assets/logo.png'
    logo=Image.open(logo_path).convert('RGBA');alpha=logo.convert('L');logo.putalpha(alpha);logo.thumbnail((220,220),Image.Resampling.LANCZOS)
    image.alpha_composite(logo,((1024-logo.width)//2,0))
    draw.text((512,251),'SWAP PRISM × ROCKET LEAGUE',font=small,fill='#68DAF9',anchor='mm')
    draw.rounded_rectangle((135,307,889,431),radius=22,fill=(20,25,32,230),outline='#68DAF9',width=2)
    draw.text((512,365),f'SEASON {season}',font=font,fill='white',anchor='mm')
    stream=io.BytesIO();image.save(stream,format='PNG');flat=pkg.flat();index=pkg.find_asset('UI_MainMenu_Logo.Event_Logo_RadSummer');exp=pkg.exports[index]
    if pkg.class_name(exp)!='Texture2D':raise ValueError('El inicio del juego ha cambiado.')
    serial=flat[i64(exp,36):i64(exp,36)+i32(exp,32)]
    replacements={index:inline_serial(pkg,serial,stream.getvalue())}
    movie_index=pkg.find_asset('GFX_StartMenu.StartMenu');movie_exp=pkg.exports[movie_index]
    data,props=export_properties(pkg,flat,movie_exp);raw=next((v for v in props if v['name']=='RawData' and v['kind']=='ArrayProperty'),None)
    if not raw:raise ValueError('No se reconoce la película del inicio.')
    from start_logo import patch_movie
    movie,proof=patch_movie(data[raw['value']+4:raw['value']+raw['size']])
    serial=bytearray(data[:raw['value']]+struct.pack('<I',len(movie))+movie+data[raw['value']+raw['size']:])
    struct.pack_into('<i',serial,raw['value']-8,len(movie)+4);replacements[movie_index]=bytes(serial)
    crc=next((v for v in props if v['name'] in ['RawDataCRC','SourceFileCRC'] and v['size']==4),None)
    if crc:
        import zlib
        position=crc['value']+(len(movie)+4-raw['size'] if crc['value']>raw['value'] else 0)
        struct.pack_into('<I',serial,position,zlib.crc32(movie));replacements[movie_index]=bytes(serial)
    return append_exports(pkg,replacements,swf_validator=lambda i,b:i==movie_index and b==replacements[i])

def make_job(api,name,raw,source,label,validation,kind):
    job=api.ROOT/'paquetes'/uuid.uuid4().hex;(job/'generado').mkdir(parents=True)
    (job/'generado'/name).write_bytes(raw)
    item={'Product':label,'AssetPackage':name,'Slot':'Personalización'}
    report={'kind':kind,'target':item,'donor':item,'original_sha256':{name:sha(source)},'generated_sha256':hashlib.sha256(raw).hexdigest(),'validation':validation}
    (job/'informe.json').write_text(json.dumps(report,indent=2),encoding='utf8');return job
