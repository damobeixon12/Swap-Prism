"""Conservative baked material colors. Never interprets arbitrary vectors as RGB.

UE3 property-stream research: c1yq/RL-Swapper, MIT.
Only named material color parameters are supported; effects may override these.
"""
import math
import struct
from modern_upk import i32, i64, Package, crypt, put32, put64
import zlib

PAINTS = {'Original':None, 'Blanco':(.880,.880,.885), 'Negro':(.012,.012,.013),
          'Rojo':(.680,.018,.018), 'Azul cielo':(.035,.450,.880),
          'Cobalto':(.025,.100,.720), 'Morado':(.260,.022,.580),
          'Rosa':(.850,.160,.380), 'Verde bosque':(.018,.300,.035),
          'Lima':(.240,.820,.015), 'Azafrán':(.880,.620,.015),
          'Naranja':(.880,.200,.008), 'Siena':(.180,.032,.006), 'Gris':(.190,.195,.205)}
SPATIAL = ('uv','tiling','position','offset','scale','rotation','speed','direction',
           'parallax','bounds','size','coord','width','height','radius','velocity',
           'accel','gravity','pivot','normal','panning','distort','brightness','intensity','opacity','alpha')


def color_name(name):
    low=name.casefold()
    return any(word in low for word in ('color','colour','tint')) and not any(word in low for word in SPATIAL)


def properties(pkg, data, start):
    """Return a complete bounded tagged stream, or reject it."""
    def name(pos):
        idx,num=struct.unpack_from('<ii',data,pos)
        if not 0<=idx<len(pkg.names) or not 0<=num<=2048: raise ValueError('FName')
        return pkg.names[idx]
    result=[]; pos=start
    try:
        for _ in range(256):
            key=name(pos)
            if key=='None': return pos+8,result
            kind=name(pos+8)
            size,array=struct.unpack_from('<ii',data,pos+16)
            if i32(data,pos+12)!=0 or size<0 or not 0<=array<=1024: return None
            value=pos+24; st=''
            if kind=='StructProperty': st=name(value); value+=8
            elif kind=='ByteProperty': name(value); value+=8
            elif kind=='BoolProperty': size=1
            elif kind not in ('IntProperty','FloatProperty','NameProperty','ObjectProperty','ComponentProperty',
                              'InterfaceProperty','ArrayProperty','StrProperty','DelegateProperty'): return None
            if value+size>len(data): return None
            result.append(dict(name=key,kind=kind,struct=st,size=size,value=value))
            pos=value+size
    except (ValueError,IndexError,struct.error): pass
    return None


def material_color_ranges(pkg, flat=None):
    flat=pkg.flat() if flat is None else flat
    found={}
    for index,exp in enumerate(pkg.exports):
        if pkg.class_name(exp)!='MaterialInstanceConstant': continue
        offset,size=i64(exp,36),i32(exp,32)
        data=flat[offset:offset+size]
        parsed=None
        for start in range(0,min(64,len(data)),4):
            parsed=properties(pkg,data,start)
            if parsed and parsed[1]: break
        if not parsed: continue
        for prop in parsed[1]:
            if prop['name']!='VectorParameterValues' or prop['kind']!='ArrayProperty' or prop['size']<4: continue
            base=prop['value']; arr=data[base:base+prop['size']]; count=i32(arr,0)
            if not 0<=count<=128: continue
            cursor=4; candidates=[]
            for _ in range(count):
                element=properties(pkg,arr,cursor)
                if not element: candidates=[]; break
                cursor,fields=element
                pn=next((p for p in fields if p['name']=='ParameterName' and p['kind']=='NameProperty' and p['size']==8),None)
                pv=next((p for p in fields if p['name']=='ParameterValue' and p['struct']=='LinearColor' and p['size']==16),None)
                if not pn or not pv: continue
                ni=i32(arr,pn['value'])
                if not 0<=ni<len(pkg.names) or not color_name(pkg.names[ni]): continue
                absolute=offset+base+pv['value']
                rgb=struct.unpack_from('<fff',flat,absolute)
                if all(math.isfinite(x) and abs(x)<=10000 for x in rgb):
                    candidates.append((absolute,{'export':pkg.path(index),'parameter':pkg.names[ni]}))
            if cursor<=len(arr): found.update(candidates)
    return found

def export_properties(pkg,flat,exp):
    data=flat[i64(exp,36):i64(exp,36)+i32(exp,32)]
    for start in list(range(0,min(64,len(data)),4))+[i for i in range(min(64,len(data))) if i%4]:
        parsed=properties(pkg,data,start)
        if parsed and parsed[1]:return data,parsed[1]
    return data,[]

def nested_fields(pkg,data,fields,base=0,depth=0):
    if depth>6:return
    for p in fields:
        yield p,base+p['value']
        if p['kind']=='StructProperty':
            sub=data[p['value']:p['value']+p['size']];parsed=properties(pkg,sub,0)
            if parsed and parsed[1]:yield from nested_fields(pkg,sub,parsed[1],base+p['value'],depth+1)
        elif p['kind']=='ArrayProperty' and p['size']>=4:
            arr=data[p['value']:p['value']+p['size']];count=i32(arr,0);cursor=4
            if not 0<=count<=512:continue
            for _ in range(count):
                parsed=properties(pkg,arr,cursor)
                if not parsed or not parsed[1]:break
                cursor,subfields=parsed
                yield from nested_fields(pkg,arr,subfields,base+p['value'],depth+1)

def particle_color_ranges(pkg,flat):
    found={};dependencies={}
    for index,exp in enumerate(pkg.exports):
        cls=pkg.class_name(exp)
        if not cls.startswith('ParticleModuleColor'):continue
        scale=cls=='ParticleModuleColorScaleOverLife'
        data,props=export_properties(pkg,flat,exp);base=i64(exp,36)
        for prop in props:
            if prop['struct']!='RawDistributionVector':continue
            raw=data[prop['value']:prop['value']+prop['size']];sub=properties(pkg,raw,0)
            if not sub:continue
            for field in sub[1]:
                pos=field['value']
                if field['name']=='Distribution' and field['size']==4 and i32(raw,pos)>0:
                    ref=i32(raw,pos)-1;dependencies[ref]=dependencies.get(ref,True) and scale
                if field['name']=='LookupTable' and field['kind']=='ArrayProperty' and field['size']>=24:
                    count=i32(raw,pos)
                    if count>=5 and (count-2)%3==0 and 4+count*4==field['size']:
                        for n in range((count-2)//3):
                            at=base+prop['value']+pos+12+n*12
                            found[at]={'export':pkg.path(index),'parameter':prop['name']+'.LookupTable','particle':True,'neutral_multiplier':scale,
                                       'bounds':base+prop['value']+pos+4}
    for index in dependencies:
        exp=pkg.exports[index]
        if not pkg.class_name(exp).startswith('DistributionVector'):continue
        data,props=export_properties(pkg,flat,exp);base=i64(exp,36)
        for p in props:
            if p['struct']=='Vector' and p['size']==12 and p['name'] in ('Constant','Min','Max','MaxOutput'):
                found[base+p['value']]={'export':pkg.path(index),'parameter':p['name'],'particle':True,'distribution':index,'neutral_multiplier':dependencies[index]}
        for p,at in nested_fields(pkg,data,props):
            if p['struct']=='Vector' and p['size']==12 and p['name']=='OutVal':
                found[base+at]={'export':pkg.path(index),'parameter':'ColorCurve.OutVal','particle':True,'distribution':index,'neutral_multiplier':dependencies[index]}
    return found,dependencies

def color_ranges(pkg,flat=None):
    flat=pkg.flat() if flat is None else flat
    found=material_color_ranges(pkg,flat);particles,_=particle_color_ranges(pkg,flat);found.update(particles)
    overrides,_=override_color_ranges(pkg,flat);found.update(overrides)
    # Explicitly named shader constants are also colour sources, never arbitrary vectors.
    for index,exp in enumerate(pkg.exports):
        cls=pkg.class_name(exp);data,props=export_properties(pkg,flat,exp);base=i64(exp,36)
        parameter=next((pkg.names[i32(data,p['value'])] for p in props if p['name']=='ParameterName' and p['size']==8),'')
        for p in props:
            allowed=(p['name']=='CarBoostGlowColor' and p['size']==16) or (cls=='MaterialExpressionVectorParameter' and color_name(parameter) and p['name']=='DefaultValue' and p['struct']=='LinearColor' and p['size']==16)
            if allowed:found[base+p['value']]={'export':pkg.path(index),'parameter':parameter or p['name']}
    return {pos:record for pos,record in found.items() if all(math.isfinite(x) and abs(x)<=10000 for x in struct.unpack_from('<fff',flat,pos))}

def override_color_ranges(pkg,flat):
    found={};controls=set()
    for index,exp in enumerate(pkg.exports):
        cls=pkg.class_name(exp)
        is_particle=cls=='ProductOverride_ParticleSystemColorParameter_TA'
        if not is_particle and cls not in ('ProductOverride_MaterialParameter_TA','ProductOverride_ParticleSystemVectorParameter_TA'):continue
        data,props=export_properties(pkg,flat,exp);base=i64(exp,36)
        for p in props:
            if p['kind']!='ArrayProperty' or p['size']<4:continue
            arr=data[p['value']:p['value']+p['size']];count=i32(arr,0);cursor=4
            if not 0<=count<=512:continue
            for _ in range(count):
                parsed=properties(pkg,arr,cursor)
                if not parsed or not parsed[1]:break
                cursor,fields=parsed
                key=next((pkg.names[i32(arr,f['value'])] for f in fields if f['kind']=='NameProperty' and f['size']==8 and f['name'] in ('PaintParameterName','ParameterName','Key')),'')
                if not is_particle and not color_name(key):continue
                edited=False
                for f in fields:
                    if f['name'] in ('CustomColor','ParameterValue','Value') and ((f['struct']=='LinearColor' and f['size']==16) or (f['struct']=='Vector' and f['size']==12)):
                        found[base+p['value']+f['value']]={'export':pkg.path(index),'parameter':key,'runtime_override':True,'particle':is_particle};edited=True
                if edited and is_particle:
                    for f in fields:
                        if f['name']=='Paint' and f['kind']=='ObjectProperty' and f['size']==4:controls.add(base+p['value']+f['value'])
    return found,controls

def disable_color_overrides(pkg,before,result,dependencies):
    """Stop runtime particle/paint parameters from replacing baked custom RGB."""
    changed=[];permitted=set()
    for index,exp in enumerate(pkg.exports):
        cls=pkg.class_name(exp)
        if cls!='ProductAttribute_PaintSettings_TA' and index not in dependencies:continue
        data,props=export_properties(pkg,before,exp);base=i64(exp,36)
        for p,relative in nested_fields(pkg,data,props):
            at=base+relative
            if p['kind']=='NameProperty' and p['size']==8 and p['name'] in ('ParameterName','PaintParameterName') and 'None' in pkg.names:
                struct.pack_into('<ii',result,at,pkg.names.index('None'),0);permitted.update(range(at,at+8));changed.append(pkg.path(index)+'.'+p['name'])
            if cls=='ProductAttribute_PaintSettings_TA' and p['kind']=='BoolProperty' and p['name'] in ('bPaintParticles','bPaintBody','bPaintWheels','bEnabled'):
                result[at]=0;permitted.add(at);changed.append(pkg.path(index)+'.'+p['name'])
    return changed,permitted


def repack(pkg, flat):
    """Recompress unchanged-size chunks; preserve metadata and trailing bytes."""
    if len(flat)!=len(pkg.flat()): raise ValueError('Tamaño de contenido distinto.')
    ordered=sorted(pkg.chunks,key=lambda c:c[2])
    if ordered!=pkg.chunks or any(a[2]+a[3]>b[2] for a,b in zip(ordered,ordered[1:])):
        raise ValueError('Distribución de chunks no compatible con pintura.')
    header=bytearray(pkg.header)
    first=ordered[0][2]; output=bytearray(pkg.raw[:first]); previous=first
    for index,(uoff,usize,coff,csize,nonce) in enumerate(ordered):
        output.extend(pkg.raw[previous:coff])
        blocks=[flat[p:min(p+131072,uoff+usize)] for p in range(uoff,uoff+usize,131072)]
        encoded=[zlib.compress(block) for block in blocks]
        payload=struct.pack('<Iiii',0x9E2A83C1,131072,sum(map(len,encoded)),usize)
        payload+=b''.join(struct.pack('<ii',len(comp),len(raw)) for comp,raw in zip(encoded,blocks))+b''.join(encoded)
        if pkg.nonce is not None: payload=crypt(pkg.key,payload,nonce)
        pos=pkg.meta.compressed_chunks_offset+4+index*pkg.stride
        put64(header,pos+12,len(output)); put32(header,pos+20,len(payload))
        output.extend(payload); previous=coff+csize
    output.extend(pkg.raw[previous:])
    header.extend(bytes(pkg.enc_aligned-len(header)))
    off=pkg.summary.name_offset
    output[off:off+pkg.enc_aligned]=crypt(pkg.key,bytes(header),pkg.nonce)
    checked=Package(pkg.file,pkg.engine,raw=bytes(output))
    if checked.flat()!=flat: raise ValueError('La verificación de color no coincide.')
    return checked


def paint(pkg, color,cooked=None):
    if color not in PAINTS or PAINTS[color] is None: raise ValueError('Color no válido.')
    before=pkg.flat(); result=bytearray(before); ranges=color_ranges(pkg,before)
    from color_textures import diffuse_textures,neutralize_diffuse
    diffuse=diffuse_textures(pkg)
    if not any(not r.get('neutral_multiplier') for r in ranges.values()) and not diffuse: raise ValueError('Este paquete no tiene fuentes de color principal editables verificadas.')
    for offset in ranges:
        old=struct.unpack_from('<fff',before,offset)
        if ranges[offset].get('neutral_multiplier'):
            value=max(0,max(old));struct.pack_into('<fff',result,offset,value,value,value)
        else:
            hdr=min(6,max(1,max(abs(x) for x in old)))
            struct.pack_into('<fff',result,offset,*(x*hdr for x in PAINTS[color]))
    permitted={p for offset in ranges for p in range(offset,offset+12)}
    # Lookup-table min/max are bounds on RGB, not time or spatial data.
    bounds={record['bounds'] for record in ranges.values() if 'bounds' in record}
    for base in bounds:
        offsets=[pos for pos,record in ranges.items() if record.get('bounds')==base]
        values=[v for pos in offsets for v in struct.unpack_from('<fff',result,pos)]
        struct.pack_into('<ff',result,base,min(values),max(values));permitted.update(range(base,base+8))
    _,dependencies=particle_color_ranges(pkg,before)
    neutralized,controls=disable_color_overrides(pkg,before,result,dependencies);permitted.update(controls)
    _,paint_refs=override_color_ranges(pkg,before)
    for at in paint_refs:struct.pack_into('<i',result,at,0);permitted.update(range(at,at+4))
    if any(a!=b and i not in permitted for i,(a,b) in enumerate(zip(before,result))):
        raise ValueError('Cambio fuera de los parámetros RGB.')
    colored=repack(pkg,bytes(result)) if result!=before else pkg
    colored,textures=neutralize_diffuse(colored,cooked,color,any(not r.get('neutral_multiplier') for r in ranges.values())) if diffuse else (colored,[])
    return colored,{'name':color,'rgb_parameters':len(ranges),'particle_rgb_parameters':sum(bool(r.get('particle')) for r in ranges.values()),
        'only_color_data_changed':True,'neutralized_color_multipliers':sum(bool(r.get('neutral_multiplier')) for r in ranges.values()),
        'runtime_paint_references_cleared':len(paint_refs),'runtime_rgb_overrides':sum(bool(r.get('runtime_override')) for r in ranges.values()),
        'diffuse_textures_neutralized_or_baked':textures,'runtime_overrides_neutralized':neutralized,'visual_verified':False,'parameters':list(ranges.values())}
