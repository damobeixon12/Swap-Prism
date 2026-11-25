"""Append-only Texture2D serial replacement. Original non-texture exports stay byte exact.

UE3 inline mip research: c1yq/RL-Swapper, MIT. No TFC/physics/process edits.
"""
from pathlib import Path
import io,json,struct,zlib,uuid
from PIL import Image,ImageOps
from modern_upk import Package,i32,i64,put32,put64,crypt,rewire
from cosmetic_colors import properties
from swap_manager import pristine,sha
from menu_guard import require_editable

METER_COMPANIONS=('GFxShell_SF.upk','GFx_ReplayHud_SF.upk','GFX_SpectatorHud_SF.upk')
# Every game mode (training, tutorial, LTMs...) ships its own copy of Ball_Default00_D/N/RGB.
BALL_COMPANIONS=tuple('GameInfo_'+m+'_SF.upk' for m in (
    'Basketball','BladeBall','Breakout','FTE','FootBall','GodBall','HeatseekerTerritory','Hops','Items','KeepUp','KnockOut',
    'LTM_AprilFool','LTM_BeachBall','LTM_BoomerBall','LTM_Demolition','LTM_DropshotRumble','LTM_Eggstra','LTM_GForce',
    'LTM_Moonball','LTM_Pinball','LTM_SpeedDemon','LTM_SpikeRush','LTM_SuperCube','MagnusFutball','Possession','Season',
    'SnowDayTerritory','SpikeDrop','Territory','TrainingEditor','Tutorial'))+('Gameinfo_Hockey_SF.upk',)

def generate_ball_companions(api,game,pack,output):
    """Same ball texture in every mode's package; skips packages another change already uses."""
    from swap_manager import load_state
    cooked=Path(game)/'TAGame/CookedPCConsole';state=load_state(api.ROOT/'data');jobs=[]
    for name in BALL_COMPANIONS:
        entry=state.get(name)
        if not (cooked/name).exists() or (entry and entry.get('status')=='installed' and not entry.get('hud_display_companion')):continue
        job=generate(api,game,api.DEFAULT_ENGINE,pack,output,package=name)
        report=json.loads((job/'informe.json').read_text(encoding='utf-8'));report['hud_display_companion']=True
        report['target']['Product']='Pelota · '+name[9:-7]
        (job/'informe.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8');jobs.append(job)
    return jobs

def generate_meter_companions(api,game,pack,output):
    """The shared code library can define BoostMeterView before the HUD movie.
    Prepare the same display-only change for each verified native class copy.
    All outputs are ordinary tracked transaction jobs with original backups.
    """
    from hud_abc import abc_tag,neutral_boostmeter_gfx
    from cosmetic_colors import export_properties
    from profile_manager import load
    from swap_manager import load_state
    require_editable();cooked=Path(game)/'TAGame/CookedPCConsole';profile=load(api.ROOT);state=load_state(api.ROOT/'data');jobs=[]
    for name in METER_COMPANIONS:
        source=pristine(cooked,name,api.ROOT/'data')
        entry=profile.get('packages',{}).get(name)
        underlying=entry.get('base_state') if entry else state.get(name)
        if underlying and underlying.get('status')=='installed' and not underlying.get('hud_display_companion'):
            raise ValueError('Otro cambio usa la biblioteca del medidor: '+name)
        pkg=Package(source,api.DEFAULT_ENGINE);flat=pkg.flat();replacements={};reports=[]
        for i,e in enumerate(pkg.exports):
            if pkg.class_name(e)!='SwfMovie':continue
            data,props=export_properties(pkg,flat,e);raw=next((p for p in props if p['name']=='RawData' and p['kind']=='ArrayProperty'),None)
            if not raw:continue
            at=raw['value']+4;old=data[at:at+raw['size']-4]
            try:abc=abc_tag(old)[-1]
            except ValueError:continue
            if 'tagame.hud.BoostMeterView' not in abc.classes:continue
            new,report=neutral_boostmeter_gfx(old);reports.append(report)
            serial=bytearray(data[:at]+new+data[at+len(old):]);put32(serial,raw['value']-8,4+len(new));put32(serial,raw['value'],len(new))
            crc=next((p for p in props if p['name'] in ('SourceFileCRC','RawDataCRC') and p['size']==4),None)
            if crc:struct.pack_into('<I',serial,crc['value']+(len(new)-len(old) if crc['value']>at else 0),zlib.crc32(new))
            replacements[i]=bytes(serial)
        if len(reports)!=1:raise ValueError('La copia compartida del medidor cambió: '+name)
        result=append_exports(pkg,replacements,swf_validator=lambda i,serial:i in replacements and serial==replacements[i])
        job=Path(output)/uuid.uuid4().hex;(job/'generado').mkdir(parents=True);generated=job/'generado'/name;generated.write_bytes(result)
        report={'schema':3,'kind':'texture','hud_display_companion':True,
                'target':{'ID':None,'Product':'Boostmeter · biblioteca compartida','Slot':'Texture','AssetPackage':name},
                'donor':{'ID':None,'Product':pack.name,'Slot':'Texture'},'original_sha256':{name:sha(source)},'generated_sha256':sha(generated),
                'validation':{'texture_and_hud_display_only':True,'other_payloads_unchanged':True,'hud_display_color':reports[0],'visual_verified':False}}
        (job/'informe.json').write_text(json.dumps(report,indent=2));jobs.append(job)
    return jobs

def parse_props(pkg,data):
    for start in list(range(0,min(64,len(data)),4))+[i for i in range(min(64,len(data))) if i%4]:
        parsed=properties(pkg,data,start)
        if parsed and parsed[1]:return start,*parsed
    raise ValueError('No se identifica la estructura de la textura.')

def mip_table(data,end):
    for off in range(end,min(end+40,len(data)-32),4):
        count=i32(data,off)
        if not 1<=count<=16:continue
        walk=off+4; levels=[]
        try:
            for _ in range(count):
                flags,usize,disk=struct.unpack_from('<Iii',data,walk)
                if usize<0 or (disk<0 and not flags&0x20):raise ValueError()
                step=28 if flags&0x21 or disk==0 else 20+disk
                w,h=struct.unpack_from('<ii',data,walk+step-8)
                if not 1<=w<=8192 or not 1<=h<=8192 or walk+step>len(data):raise ValueError()
                levels.append((walk,flags,disk,w,h));walk+=step
            return off,walk,levels
        except (ValueError,struct.error):pass
    raise ValueError('Mips no reconocidos: no se modifica el paquete.')

def inline_serial(pkg,data,raw,fit=False):
    """fit=True crops the picture to the texture's proportions instead of stretching it."""
    start,end,props=parse_props(pkg,data); off,tail,levels=mip_table(data,end)
    fmt=next((pkg.names[i32(data,p['value'])] for p in props if p['name']=='Format' and p['size']==8),'')
    if fmt not in ('PF_DXT1','PF_DXT5','PF_A8R8G8B8'):raise ValueError('Formato de textura no admitido: '+fmt)
    if 'None' not in pkg.names:raise ValueError('Falta el terminador del paquete.')
    with Image.open(io.BytesIO(raw)) as original:
        image=original.convert('RGBA')
    # Preserve existing dimensions/aspect and pixel format, including normal-map encoding.
    sx=next((i32(data,p['value']) for p in props if p['name']=='SizeX'),levels[0][3])
    sy=next((i32(data,p['value']) for p in props if p['name']=='SizeY'),levels[0][4])
    if not 1<=sx<=4096 or not 1<=sy<=4096:raise ValueError('Resolución no admitida.')
    base=ImageOps.fit(image,(sx,sy),Image.Resampling.LANCZOS) if fit else image.resize((sx,sy),Image.Resampling.LANCZOS)
    header=bytearray(data[:start]);previous=start;never=False
    for p in props:
        stop=p['value']+p['size']; part=bytearray(data[previous:stop]);rel=p['value']-previous;previous=stop
        if p['name'] in ('TextureFileCacheName','FirstResourceMemMip'):continue
        if p['name']=='NeverStream':part[rel]=1;never=True
        if p['name']=='LODBias' and p['size']==4:put32(part,rel,0)
        header.extend(part)
    if not never and 'NeverStream' in pkg.names and 'BoolProperty' in pkg.names:
        header.extend(struct.pack('<iiiiiiB',pkg.names.index('NeverStream'),0,pkg.names.index('BoolProperty'),0,0,0,1))
    header.extend(struct.pack('<ii',pkg.names.index('None'),0));header.extend(data[end:off])
    no_mips=any(p['name']=='MipGenSettings' and p['size']==8 and pkg.names[i32(data,p['value'])]=='TMGS_NoMipmaps' for p in props)
    chunks=[];w,h=sx,sy
    while True:
        level=base.resize((w,h),Image.Resampling.LANCZOS)
        if fmt=='PF_A8R8G8B8':pixels=level.tobytes('raw','BGRA');mw,mh=w,h
        else:
            mw,mh=max(4,w),max(4,h)
            level=level.resize((mw,mh),Image.Resampling.LANCZOS)
            target=io.BytesIO();level.save(target,format='DDS',pixel_format=fmt.removeprefix('PF_'))
            pixels=target.getvalue()[128:]
        chunks.append(struct.pack('<Iii',0x10000,len(pixels),len(pixels))+pixels+struct.pack('<ii',mw,mh))
        if no_mips or (w==1 and h==1):break
        w,h=max(1,w//2),max(1,h//2)
    return bytes(header)+struct.pack('<i',len(chunks))+b''.join(chunks)+data[tail:]

def append_exports(pkg,replacements,swf_validator=None,body_validator=None):
    before=pkg.flat();flat=bytearray(before);header=bytearray(pkg.header)
    export_cursor=pkg.summary.export_offset-pkg.summary.name_offset
    for index,exp in enumerate(pkg.exports):
        if index in replacements:
            blob=replacements[index];put32(header,export_cursor+32,len(blob));put64(header,export_cursor+36,len(flat));flat.extend(blob)
        export_cursor+=len(exp)
    # Only extend the last virtual chunk, preserving every original object address.
    virtual_last=max(range(len(pkg.chunks)),key=lambda i:pkg.chunks[i][0])
    last=pkg.chunks[virtual_last]
    if last[0]+last[1]!=len(before):raise ValueError('Cola de chunks no compatible.')
    pos=pkg.meta.compressed_chunks_offset+4+virtual_last*pkg.stride
    put32(header,pos+8,last[1]+len(flat)-len(before))
    ordered=sorted(enumerate(pkg.chunks),key=lambda v:v[1][2]);out=bytearray(pkg.raw[:ordered[0][1][2]]);previous=ordered[0][1][2]
    for index,(uoff,usize,coff,csize,nonce) in ordered:
        if index==virtual_last:usize+=len(flat)-len(before)
        if coff<previous:raise ValueError('Chunks solapados.')
        out.extend(pkg.raw[previous:coff])
        if index!=virtual_last:
            payload=pkg.raw[coff:coff+csize]
        else:
            blocks=[bytes(flat[p:min(p+131072,uoff+usize)]) for p in range(uoff,uoff+usize,131072)]
            reusable=[]
            try:
                old_payload=pkg.raw[coff:coff+csize]
                if pkg.nonce is not None:old_payload=crypt(pkg.key,old_payload,nonce,decrypt=True)
                total=i32(old_payload,12);cursor=16;amount=0;entries=[]
                while amount<total:
                    compressed,uncompressed=struct.unpack_from('<ii',old_payload,cursor);cursor+=8
                    if compressed<=0 or uncompressed<=0:raise ValueError('Bloque inválido')
                    entries.append((compressed,uncompressed));amount+=uncompressed
                for compressed,uncompressed in entries:
                    reusable.append((old_payload[cursor:cursor+compressed],uncompressed));cursor+=compressed
            except (ValueError,struct.error):reusable=[]
            encoded=[]
            for n,block in enumerate(blocks):
                candidate=reusable[n] if n<len(reusable) else None
                if candidate and candidate[1]==len(block) and zlib.decompress(candidate[0])==block:encoded.append(candidate[0])
                else:encoded.append(zlib.compress(block,3))
            payload=struct.pack('<Iiii',0x9E2A83C1,131072,sum(map(len,encoded)),usize)+b''.join(struct.pack('<ii',len(c),len(b)) for c,b in zip(encoded,blocks))+b''.join(encoded)
            if pkg.nonce is not None:payload=crypt(pkg.key,payload,nonce)
        pos=pkg.meta.compressed_chunks_offset+4+index*pkg.stride
        put64(header,pos+12,len(out));put32(header,pos+20,len(payload));out.extend(payload);previous=coff+csize
    out.extend(pkg.raw[previous:]);header.extend(bytes(pkg.enc_aligned-len(header)));off=pkg.summary.name_offset
    out[off:off+pkg.enc_aligned]=crypt(pkg.key,bytes(header),pkg.nonce)
    checked=Package(pkg.file,pkg.engine,raw=bytes(out));after=checked.flat()
    for index,(old,new) in enumerate(zip(pkg.exports,checked.exports)):
        expected=replacements.get(index,before[i64(old,36):i64(old,36)+i32(old,32)])
        if after[i64(new,36):i64(new,36)+i32(new,32)]!=expected:raise ValueError('Falló la validación de exports.')
        if index in replacements and pkg.class_name(old) not in ('Texture2D','MaterialInstanceConstant'):
            if pkg.class_name(old)=='ProductAsset_Body_TA' and body_validator is not None and body_validator(index,expected):continue
            if not (pkg.class_name(old)=='SwfMovie' and ((pkg.file.name.casefold()=='gfx_hud_sf.upk' and pkg.path(index)=='GFX_Hud.HUD') or
                    (swf_validator is not None and swf_validator(index,expected)))):
                raise ValueError('Solo se admiten texturas, sus materiales y el color del boostmeter verificado.')
    return bytes(out)

def bindings(pkg,flat):
    """Material parameter name -> exact texture export. No guessing texture suffixes."""
    result={}
    for exp in pkg.exports:
        if pkg.class_name(exp)!='MaterialInstanceConstant':continue
        data=flat[i64(exp,36):i64(exp,36)+i32(exp,32)]
        try:_,_,props=parse_props(pkg,data)
        except ValueError:continue
        for p in props:
            if p['name']!='TextureParameterValues' or p['kind']!='ArrayProperty':continue
            arr=data[p['value']:p['value']+p['size']];cursor=4
            count=i32(arr,0)
            if not 0<=count<=128:continue
            for _ in range(count):
                parsed=properties(pkg,arr,cursor)
                if not parsed:break
                cursor,fields=parsed
                n=next((f for f in fields if f['name']=='ParameterName' and f['size']==8),None)
                v=next((f for f in fields if f['name']=='ParameterValue' and f['size']==4),None)
                if n and v:
                    ni=i32(arr,n['value']);idx=i32(arr,v['value'])-1
                    if 0<=ni<len(pkg.names) and 0<=idx<len(pkg.exports) and pkg.class_name(pkg.exports[idx])=='Texture2D':
                        result.setdefault(pkg.names[ni],set()).add(idx)
    return result

def add_mask_binding(pkg,flat,raw,replacements):
    """Add only an existing shader mask parameter, reusing the pack's local blank texture."""
    parameter='2_Diffuse_Skin_Mask'
    if parameter not in pkg.names:return False
    blanks=[i for i,e in enumerate(pkg.exports) if pkg.class_name(e)=='Texture2D' and pkg.export_name(e)=='T_BlackAlpha']
    if len(blanks)!=1:return False
    index=blanks[0];changed=0
    for mi,exp in enumerate(pkg.exports):
        if pkg.class_name(exp)!='MaterialInstanceConstant':continue
        data=flat[i64(exp,36):i64(exp,36)+i32(exp,32)]
        try:_,_,props=parse_props(pkg,data)
        except ValueError:continue
        for p in props:
            if p['name']!='TextureParameterValues' or p['kind']!='ArrayProperty':continue
            arr=data[p['value']:p['value']+p['size']];cursor=4;template=None;exists=False
            for _ in range(i32(arr,0)):
                start=cursor;parsed=properties(pkg,arr,cursor)
                if not parsed:return False
                cursor,fields=parsed
                n=next((f for f in fields if f['name']=='ParameterName' and f['size']==8),None)
                v=next((f for f in fields if f['name']=='ParameterValue' and f['size']==4),None)
                if n and v:
                    nm=pkg.names[i32(arr,n['value'])]
                    if nm==parameter:exists=True
                    if nm=='1_Diffuse_Skin':template=(start,cursor,n,v,fields)
            if exists or not template:continue
            start,stop,n,v,fields=template;elem=bytearray(arr[start:stop])
            put32(elem,n['value']-start,pkg.names.index(parameter));put32(elem,v['value']-start,index+1)
            for field in fields:
                if field['name']=='ExpressionGUID' and field['size']==16:
                    elem[field['value']-start:field['value']-start+16]=bytes(16)
            new=bytearray(data[:p['value']+p['size']]+elem+data[p['value']+p['size']:])
            put32(new,p['value']-8,p['size']+len(elem));put32(new,p['value'],i32(arr,0)+1)
            replacements[mi]=bytes(new);changed+=1
    if changed:
        exp=pkg.exports[index];replacements[index]=inline_serial(pkg,flat[i64(exp,36):i64(exp,36)+i32(exp,32)],raw)
    return bool(changed)

def generate(api,game,engine,pack,output,target=None,boost_texture=None,package=None):
    require_editable()
    base=None
    if pack.category=='Decal':
        rows=api.read_database(engine)
        base=next((r for r in rows if r['ID']==pack.skin_id and r.get('Slot')=='Decal' and r.get('AssetPath')),None)
        if not base:raise ValueError('El SkinID del pack no tiene un paquete verificado.')
        target=target or base
        from collections import Counter
        uses=Counter(r.get('AssetPackage') for r in rows)
        if uses[target.get('AssetPackage')]>1 or uses[base.get('AssetPackage')]>1:
            raise ValueError('El decal usa un paquete compartido. Elige otro destino.')
    if pack.category=='Pelota':
        name=package or 'GameInfo_Soccar_SF.upk';mapping={'Diffuse':'Ball_Default00_D','Normal':'Ball_Default00_N','Masks':'Ball_Default00_RGB'}
    elif pack.category=='Boostmeter':
        name='GFX_Hud_SF.upk';mapping={'Background':'BoostMeter_Background','Fill':'BoostMeter_Fill','Tint':'BoostMeter_FillTintablePortion','Glow':'BoostMeter_Glow'}
    else:
        if not target or target.get('Slot') not in ('Decal','Rocket Boost'):raise ValueError('Selecciona un decal o boost como destino.')
        name=(base or target)['AssetPackage'];mapping=None
    cooked=Path(game)/'TAGame/CookedPCConsole';source=pristine(cooked,name,api.ROOT/'data');pkg=Package(source,engine);flat=pkg.flat();replacements={};used=[]
    if pack.category=='Boost':
        matches=[i for i,e in enumerate(pkg.exports) if pkg.class_name(e)=='Texture2D' and pkg.path(i)==boost_texture]
        if len(matches)!=1:raise ValueError('Selecciona la textura exacta del boost.')
        index=matches[0];exp=pkg.exports[index]
        replacements[index]=inline_serial(pkg,flat[i64(exp,36):i64(exp,36)+i32(exp,32)],pack.layers['Diffuse']);used.append(boost_texture)
    elif mapping:
        layers=dict(pack.layers)
        if pack.category=='Boostmeter':
            # Empty AlphaConsole Tint means no team-tinted overlay, not "keep vanilla".
            if 'Tintable' in layers:layers['Tint']=layers.pop('Tintable')
            if 'Tint' not in layers:
                transparent=io.BytesIO();Image.new('RGBA',(256,256),(0,0,0,0)).save(transparent,format='PNG')
                layers['Tint']=transparent.getvalue()
        for role,raw in layers.items():
            if role not in mapping:raise ValueError('Capa no compatible: '+role)
            matches=[i for i,e in enumerate(pkg.exports) if pkg.class_name(e)=='Texture2D' and pkg.export_name(e)==mapping[role]]
            if len(matches)!=1:raise ValueError('No se localiza la textura '+mapping[role])
            index=matches[0];exp=pkg.exports[index]
            replacements[index]=inline_serial(pkg,flat[i64(exp,36):i64(exp,36)+i32(exp,32)],raw);used.append(role)
    else:
        params=bindings(pkg,flat)
        for role,raw in pack.layers.items():
            indexes=params.get(role,set())
            # AlphaConsole packs sometimes duplicate Diffuse under the actual shader slot.
            if not indexes and role=='Diffuse' and pack.layers.get('1_Diffuse_Skin')==raw and params.get('1_Diffuse_Skin'):
                continue
            if not indexes and role=='2_Diffuse_Skin_Mask' and add_mask_binding(pkg,flat,raw,replacements):
                used.append(role);continue
            if not indexes:raise ValueError('El material del destino no admite la capa '+role+'. Selecciona el decal base del pack.')
            for index in indexes:
                exp=pkg.exports[index];replacements[index]=inline_serial(pkg,flat[i64(exp,36):i64(exp,36)+i32(exp,32)],raw)
            used.append(role)
    if not replacements:raise ValueError('No hay texturas compatibles para aplicar.')
    hud_color=None
    if pack.category=='Boostmeter':
        from hud_abc import patch_boostmeter_movie
        index,serial,hud_color=patch_boostmeter_movie(pkg,flat);replacements[index]=serial
    result=append_exports(pkg,replacements)
    original_hash={name:sha(source)}
    if base and base['ID']!=target['ID']:
        target_source=pristine(cooked,target['AssetPackage'],api.ROOT/'data')
        target_pkg=Package(target_source,engine)
        painted=Package(source,engine,raw=result)
        result,_=rewire(painted,target_pkg,base,target)
        name=target['AssetPackage'];original_hash[name]=sha(target_source)
    job=Path(output)/uuid.uuid4().hex;(job/'generado').mkdir(parents=True)
    generated=job/'generado'/name;generated.write_bytes(result)
    target=target or {'ID':None,'Product':pack.category,'Slot':'Texture','AssetPackage':name}
    report={'schema':3,'kind':'texture','target':target,'donor':{'ID':None,'Product':pack.name,'Slot':'Texture'},
            'original_sha256':original_hash,'generated_sha256':sha(generated),
            'validation':{'texture_and_material_bindings_only':hud_color is None,'texture_and_hud_display_only':hud_color is not None,'other_payloads_unchanged':True,'layers':used,'exports':len(replacements),'visual_verified':False,
                          'vanilla_tint_removed':pack.category=='Boostmeter' and 'Tint' not in pack.layers and 'Tintable' not in pack.layers},
            'source_pack_sha256':pack.digest}
    report['validation']['hud_display_color']=hud_color
    (job/'informe.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    return job
