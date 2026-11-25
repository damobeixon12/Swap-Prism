"""Original-body visual bridge. Never installs a donor body package as the target.

Only Mesh/AnimTreeTemplate object references may change in the owned ProductAsset.
HandlingPreset, axles, suspension and all other original export bytes are verified.
"""
from pathlib import Path
import struct,json,uuid,hashlib
from modern_upk import Package,i32,i64,put32,put64,crypt
from cosmetic_colors import export_properties
from swap_manager import pristine,sha

VISUAL_FIELDS={'Mesh','AnimTreeTemplate'}

def import_path(pkg,reference):
    if reference>0:return pkg.path(reference-1)
    parts=[];seen=set()
    while reference<0:
        if reference in seen:raise ValueError('Import cíclico.')
        seen.add(reference);entry=pkg.imports[-reference-1]
        parts.append(pkg.fname(i32(entry,20),i32(entry,24)));reference=i32(entry,16)
    return '.'.join(reversed(parts))

def bridge_imports(pkg,specs,extra_names=()):
    names=list(pkg.names);name_bytes=bytearray();imports=list(pkg.imports)
    def name(text):
        for i,existing in enumerate(names):
            if existing.casefold()==text.casefold():return i
        raw=text.encode('windows-1252')+b'\0';idx=len(names);names.append(text)
        name_bytes.extend(struct.pack('<i',len(raw))+raw+struct.pack('<Q',pkg.name_flags[0]));return idx
    refs={}
    for value in extra_names:name(value)
    for path,cls in specs:
        outer=0;parts=path.split('.')
        if len(parts)<2:raise ValueError('Referencia visual incompleta.')
        for depth,part in enumerate(parts):
            prefix='.'.join(parts[:depth+1]);found=None
            for ix,entry in enumerate(imports):
                if i32(entry,16)==outer and names[i32(entry,20)].casefold()==part.casefold():found=-ix-1;break
            if found is None:
                klass=cls if depth==len(parts)-1 else 'Package'
                entry=struct.pack('<iiiiiii',name('Engine' if klass!='Package' else 'Core'),0,name(klass),0,outer,name(part),0)
                imports.append(entry);found=-len(imports)
            outer=found
        refs[path]=outer
    ndelta=len(name_bytes);new_imports=b''.join(imports[len(pkg.imports):]);delta=ndelta+len(new_imports)
    if not delta:return pkg,refs
    s=pkg.summary;off=s.name_offset;irel=s.import_offset-off;erel=s.export_offset-off;iend=irel+len(pkg.imports)*28
    if not pkg.names_end<=irel<=iend<=erel:raise ValueError('Tablas de cabecera no compatibles con el puente visual.')
    exports=[bytearray(e) for e in pkg.exports]
    for exp in exports:
        if i64(exp,36)>0:put64(exp,36,i64(exp,36)+delta)
    header=bytearray(pkg.header[:pkg.names_end]+name_bytes+pkg.header[pkg.names_end:iend]+new_imports+
                     pkg.header[iend:erel]+b''.join(exports)+pkg.header[pkg.exports_end:])
    chunkoffset=pkg.meta.compressed_chunks_offset+delta
    aligned=(len(header)+15)&~15;growth=aligned-pkg.enc_aligned;gap=s.total_header_size-off-pkg.enc_aligned
    extra=(max(growth-gap,delta-pkg.meta.garbage_size,0)+255)&~15 if growth>gap or delta>pkg.meta.garbage_size else 0
    for ix in range(len(pkg.chunks)):
        pos=chunkoffset+4+ix*pkg.stride;put64(header,pos,i64(header,pos)+delta);put64(header,pos+12,i64(header,pos+12)+extra)
    header.extend(bytes(aligned-len(header)));prefix=bytearray(pkg.raw[:off]);offsets=pkg.upk._find_summary_offsets(pkg.raw)
    for key,value in [('total_header_size_offset',s.total_header_size+extra),('name_count_offset',len(names)),
                      ('import_count_offset',len(imports)),('import_offset_offset',s.import_offset+ndelta),
                      ('export_offset_offset',s.export_offset+delta),('depends_offset_offset',s.depends_offset+delta)]:put32(prefix,offsets[key],value)
    for key in ('import_export_guids_offset_offset','thumbnail_table_offset_offset'):
        if i32(prefix,offsets[key])>0:put32(prefix,offsets[key],i32(prefix,offsets[key])+delta)
    pkg.upk._patch_generation_counts(prefix,offsets,s.export_count,len(names))
    put32(prefix,pkg.meta_offset,pkg.meta.garbage_size+extra-delta);put32(prefix,pkg.meta_offset+4,chunkoffset)
    oldgap=pkg.raw[off+pkg.enc_aligned:s.total_header_size];newgap=gap+extra-growth
    output=prefix+crypt(pkg.key,bytes(header),pkg.nonce)+oldgap[:newgap]+bytes(max(0,newgap-len(oldgap)))+pkg.raw[s.total_header_size:]
    checked=Package(pkg.file,pkg.engine,raw=bytes(output));before,after=pkg.flat(),checked.flat()
    for old,new in zip(pkg.exports,checked.exports):
        if before[i64(old,36):i64(old,36)+i32(old,32)]!=after[i64(new,36):i64(new,36)+i32(new,32)]:raise ValueError('El puente alteró un export original.')
    return checked,refs

def validate_visual_change(original,modified,position,fields):
    if not fields<=VISUAL_FIELDS or 'Mesh' not in fields:raise ValueError('Solo se permite modificar el modelo y la animación visual.')
    before,after=original.flat(),modified.flat()
    if len(original.exports)!=len(modified.exports):raise ValueError('Ha cambiado el conjunto de exports originales.')
    allowed=set()
    old_body=original.exports[position];_,props=export_properties(original,before,old_body)
    for p in props:
        if p['name'] in fields and p['kind']=='ObjectProperty' and p['size']==4:allowed.update(range(p['value'],p['value']+4))
    for ix,(old,new) in enumerate(zip(original.exports,modified.exports)):
        a=before[i64(old,36):i64(old,36)+i32(old,32)];b=after[i64(new,36):i64(new,36)+i32(new,32)]
        if len(a)!=len(b):raise ValueError('Cambió el tamaño de un export original.')
        changes={i for i,(x,y) in enumerate(zip(a,b)) if x!=y}
        if (ix!=position and changes) or (ix==position and not changes<=allowed):raise ValueError('Cambio fuera de las referencias visuales autorizadas.')
    invariant=bytearray(before[i64(old_body,36):i64(old_body,36)+i32(old_body,32)])
    for at in allowed:invariant[at]=0
    return {'original_body_physics_data_unchanged':True,'only_visual_body_refs_changed':True,
            'original_nonvisual_body_sha256':hashlib.sha256(invariant).hexdigest(),
            'fields':sorted(fields),'original_exports_preserved':len(original.exports),'visual_verified':False}

def physics_field(name):
    return any(v in name.casefold() for v in ('handling','physics','physasset','axle','chassisspring','suspension','collision','mass','inertia'))

def remap_property_blob(pkg,dest,data,field,refs):
    """Remap only typed FNames and object references, never opaque mesh data."""
    start=field['value']-24-(8 if field['kind'] in ('StructProperty','ByteProperty') else 0)
    stop=field['value']+field['size'];blob=bytearray(data[start:stop]);lookup={n.casefold():i for i,n in enumerate(dest.names)}
    def fname(pos):
        old=i32(blob,pos)
        if not 0<=old<len(pkg.names):raise ValueError('FName físico no válido.')
        put32(blob,pos,lookup[pkg.names[old].casefold()])
    def fields(items,base):
        for p in items:
            at=base+p['value'];header=at-24-(8 if p['kind'] in ('StructProperty','ByteProperty') else 0)
            fname(header);fname(header+8)
            if p['kind'] in ('StructProperty','ByteProperty'):fname(header+24)
            if p['kind']=='NameProperty':fname(at)
            elif p['kind'] in ('ObjectProperty','ComponentProperty'):
                ref=i32(blob,at)
                if ref:put32(blob,at,refs[import_path(pkg,ref)])
            elif p['kind']=='StructProperty':
                sub=bytes(blob[at:at+p['size']]);parsed=properties_original(pkg,data,start+at,p['size'])
                if parsed:
                    fields(parsed,at);fname(at+p['size']-8)
    def properties_original(pkg,data,pos,size):
        from cosmetic_colors import properties
        parsed=properties(pkg,data[pos:pos+size],0)
        return parsed[1] if parsed and parsed[0]==size else None
    fields([{**field,'value':field['value']-start}],0)
    return bytes(blob)

def generate(api,engine,game,output,target,donor,version='',log=print):
    """Keep the donor's render dependencies in-package; transplant original handling.

    External imports into another seek-free body package resolve in the editor
    but are not guaranteed loaded in-game. That was the invisible-car failure.
    """
    from modern_upk import rewire
    from texture_engine import append_exports
    from cosmetic_colors import properties
    cooked=Path(game)/'TAGame/CookedPCConsole';source=pristine(cooked,target['AssetPackage'],api.ROOT/'data')
    other=Package(pristine(cooked,donor['AssetPackage'],api.ROOT/'data'),engine);original=Package(source,engine)
    opos=original.find_asset(target['AssetPath']);dpos=other.find_asset(donor['AssetPath'])
    if any(p.class_name(p.exports[i])!='ProductAsset_Body_TA' for p,i in [(original,opos),(other,dpos)]):raise ValueError('Carrocería no verificada.')
    odata,oprops=export_properties(original,original.flat(),original.exports[opos]);physical=[p for p in oprops if physics_field(p['name'])]
    handling=next((p for p in physical if p['name']=='HandlingPreset' and p['kind']=='ObjectProperty'),None)
    if not handling:raise ValueError('No se ha identificado la hitbox original. Este par no se aplica.')
    specs=[]
    def collect(data,props):
        for p in props:
            if p['kind'] in ('ObjectProperty','ComponentProperty') and i32(data,p['value']):
                ref=i32(data,p['value']);path=import_path(original,ref)
                cls=original.class_name(original.exports[ref-1]) if ref>0 else original.names[i32(original.imports[-ref-1],8)]
                if not path.startswith('HandlingPresets.'):raise ValueError('Los datos físicos originales dependen de otro paquete. No se instala un modelo invisible.')
                specs.append((path,cls))
            elif p['kind']=='StructProperty':
                sub=data[p['value']:p['value']+p['size']];parsed=properties(original,sub,0)
                if parsed and parsed[0]==len(sub):collect(sub,parsed[1])
    collect(odata,physical)
    bridged,refs=bridge_imports(other,specs,original.names)
    flat=bridged.flat();data,props=export_properties(bridged,flat,bridged.exports[dpos]);parsed=next((properties(bridged,data,n) for n in range(0,64,4) if properties(bridged,data,n)),None)
    if not parsed:raise ValueError('No se reconoce el objeto de carrocería.')
    stream_start=props[0]['value']-24-(8 if props[0]['kind'] in ('StructProperty','ByteProperty') else 0)
    # Remove donor physics overrides (including offsets absent from the original).
    chunks=[];cursor=stream_start
    for p in props:
        stop=p['value']+p['size']
        if not physics_field(p['name']):chunks.append(data[cursor:stop])
        cursor=stop
    transferred=[remap_property_blob(original,bridged,odata,p,refs) for p in physical]
    blob=data[:stream_start]+b''.join(chunks+transferred)+data[cursor:]
    result=Package(bridged.file,engine,raw=append_exports(bridged,{dpos:blob},body_validator=lambda i,b:i==dpos and b==blob))
    raw,_=rewire(result,original,donor,target);checked=Package(source,engine,raw=raw)
    body=checked.exports[checked.find_asset(target['AssetPath'])];cdata,cprops=export_properties(checked,checked.flat(),body)
    actual=[p for p in cprops if physics_field(p['name'])]
    if [p['name'] for p in actual]!=[p['name'] for p in physical]:raise ValueError('No se conservaron todos los campos físicos originales.')
    # Reverse-normalize the typed field blobs into the original name table.
    backrefs={path:i32(odata,p['value']) for p in physical if p['kind']=='ObjectProperty' and i32(odata,p['value']) for path in [import_path(original,i32(odata,p['value']))]}
    for before,after in zip(physical,actual):
        expected=remap_property_blob(original,original,odata,before,backrefs)
        restored=remap_property_blob(checked,original,cdata,after,backrefs)
        if expected!=restored:raise ValueError('Un campo físico no coincide: '+before['name'])
    mesh=next((p for p in cprops if p['name']=='Mesh'),None)
    if not mesh or i32(cdata,mesh['value'])<=0:raise ValueError('El modelo visual no está incorporado en su paquete.')
    mesh_index=i32(cdata,mesh['value'])-1
    if checked.class_name(checked.exports[mesh_index])!='SkeletalMesh':raise ValueError('El modelo visual no es válido.')
    job=Path(output)/uuid.uuid4().hex;(job/'generado').mkdir(parents=True);file=job/'generado'/target['AssetPackage'];file.write_bytes(raw)
    validation={'original_body_physics_fields_unchanged':True,'original_handling_preset':import_path(original,i32(odata,handling['value'])),'physics_fields':[p['name'] for p in physical],'self_contained_visual_dependencies':True,'visual_verified':False}
    report={'schema':5,'kind':'body_visual','target':target,'donor':donor,'game_version':version,'original_sha256':{target['AssetPackage']:sha(source)},'generated_sha256':sha(file),'validation':validation}
    (job/'informe.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf8')
    log('Modelo incorporado, referencias visuales locales y hitbox original verificadas.');return job
