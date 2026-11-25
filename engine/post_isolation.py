"""Copy the ball-contact subtree; route ONLY the two goal-frame switches to it."""
import struct,hashlib

def validate_layout(raw,parse_chunks):
    parts=dict(parse_chunks(raw))
    if len(parts.get(b'BKHD',b''))<16:raise ValueError('Cabecera de banco incompleta.')
    alignment=struct.unpack_from('<I',parts[b'BKHD'],12)[0]&0xffff
    if not alignment or alignment>4096 or alignment&(alignment-1):raise ValueError('Alineación del banco desconocida.')
    index=parts.get(b'DIDX',b'')
    if len(index)%12:raise ValueError('Índice de muestras truncado.')
    entries=list(struct.iter_unpack('<III',index));ids=[i for i,_,_ in entries]
    if ids!=sorted(ids) or len(ids)!=len(set(ids)):raise ValueError('El índice de muestras no está ordenado o tiene IDs repetidos.')
    pos=0
    while pos<len(raw) and raw[pos:pos+4]!=b'DATA':pos+=8+struct.unpack_from('<I',raw,pos+4)[0]
    if pos>=len(raw):raise ValueError('No hay audio DATA en el banco.')
    begin=pos+8;size=len(parts[b'DATA'])
    for _,off,length in entries:
        if off+length>size:raise ValueError('Muestra fuera del banco.')
        if (begin+off)%alignment:raise ValueError('La dirección absoluta de audio no está alineada.')
    return {'alignment':alignment,'data_start':begin,'media_count':len(entries),'ids_sorted':True,'misaligned_media':0}

def pack_aligned(chunks,updated,parse_chunks):
    parts=dict(chunks);alignment=struct.unpack_from('<I',parts[b'BKHD'],12)[0]&0xffff
    if not alignment or alignment>4096 or alignment&(alignment-1):raise ValueError('Alineación del banco desconocida.')
    # Wwise uses padding at the end of BKHD (also present in native banks).
    # Growing DIDX moves DATA; adjust header padding to retain absolute alignment.
    before=0;header_seen=False
    for tag,payload in chunks:
        if tag==b'DATA':break
        if tag==b'BKHD':header_seen=True
        before+=8+len(updated.get(tag,payload))
    if not header_seen:raise ValueError('Orden de secciones desconocido.')
    padding=(-(before+8))%alignment
    updated={**updated,b'BKHD':updated.get(b'BKHD',parts[b'BKHD'])+b'\0'*padding}
    result=b''.join(tag+struct.pack('<I',len(updated.get(tag,payload)))+updated.get(tag,payload) for tag,payload in chunks)
    validate_layout(result,parse_chunks)
    return result

def objects(hirc):
    if len(hirc)<4:raise ValueError('HIRC truncado.')
    pos=4;rows=[];seen=set()
    for _ in range(struct.unpack_from('<I',hirc)[0]):
        if pos+9>len(hirc):raise ValueError('Objeto de audio truncado.')
        size=struct.unpack_from('<I',hirc,pos+1)[0]+5
        if size<9 or pos+size>len(hirc):raise ValueError('Tamaño de audio no válido.')
        ident=struct.unpack_from('<I',hirc,pos+5)[0]
        if ident in seen:raise ValueError('Identificador de audio repetido.')
        seen.add(ident);rows.append((ident,pos,hirc[pos:pos+size]));pos+=size
    if pos!=len(hirc):raise ValueError('Datos HIRC fuera de objetos.')
    return rows

def isolate(raw,spec,pcm,parse_chunks):
    if hashlib.sha256(raw).hexdigest()!=spec['original_sha256']:raise ValueError('El banco original ha cambiado.')
    route=spec['isolation'];chunks=parse_chunks(raw);parts=dict(chunks);hirc=parts[b'HIRC'];old=objects(hirc);by_id={i:b for i,_,b in old}
    used=set(route['reserved_ids'])|set(by_id)
    def allocate(label):
        value=int.from_bytes(hashlib.sha256((spec['original_sha256']+label).encode()).digest()[:4],'little')
        while not value or value in used:value=(value+1)&0xffffffff
        used.add(value);return value
    ids={n['id']:allocate('post-node-'+str(n['id'])) for n in route['nodes']}
    media_ids={n['media']['sourceID']['value'] for n in route['nodes'] if 'media'in n}
    new_media={i:allocate('post-media-'+str(i)) for i in sorted(media_ids)}
    validate_layout(raw,parse_chunks)
    data=bytearray(parts[b'DATA']);entries=list(struct.iter_unpack('<III',parts[b'DIDX']))
    original_entries={i:(off,size) for i,off,size in entries}
    available=set(original_entries)
    if not media_ids<=available:raise ValueError('Falta una muestra original del poste.')
    for ident in sorted(media_ids):
        while len(data)%16:data.append(0)
        entries.append((new_media[ident],len(data),len(pcm)));data.extend(pcm)
    entries.sort(key=lambda row:row[0]);index=b''.join(struct.pack('<III',*row) for row in entries)
    clones=[]
    # Preserve the native serialized child-before-parent order, not numeric IDs.
    for node in sorted(route['nodes'],key=lambda item:item['start']):
        serial=by_id[node['id']]
        if len(serial)!=node['size'] or hashlib.sha256(serial).hexdigest()!=node['sha256']:raise ValueError('La rama de audio cambió.')
        new=bytearray(serial)
        for ref in node['refs']:
            if struct.unpack_from('<I',new,ref['offset'])[0]!=ref['value']:raise ValueError('Referencia de audio no verificada.')
            struct.pack_into('<I',new,ref['offset'],ids[ref['value']])
        if 'media' in node:
            params=node['media']
            if params['StreamType']['value']!=0:raise ValueError('La muestra del poste no está incorporada.')
            for key,value in [('sourceID',new_media[params['sourceID']['value']]),('ulPluginID',0x10001),('uInMemoryMediaSize',len(pcm))]:
                ref=params[key]
                if struct.unpack_from('<I',new,ref['offset'])[0]!=ref['value']:raise ValueError('Formato de muestra no verificado.')
                struct.pack_into('<I',new,ref['offset'],value)
        clones.append(bytes(new))
    start=route['parent_start'];parent=by_id[route['parent']]
    if len(parent)!=route['parent_size'] or hashlib.sha256(parent).hexdigest()!=route['parent_sha256']:raise ValueError('El selector de superficies cambió.')
    new=bytearray(parent)
    for ref in route['post_refs']:
        if struct.unpack_from('<I',new,ref['offset']-start)[0]!=route['root']:raise ValueError('El destino de poste cambió.')
        struct.pack_into('<I',new,ref['offset']-start,ids[route['root']])
    for key in ['child_count','param_count']:
        field=route[key];at=field['offset']-start
        if struct.unpack_from('<I',new,at)[0]!=field['value']:raise ValueError('El selector de sonido tiene otra estructura.')
        struct.pack_into('<I',new,at,field['value']+1)
    a,b=route['param_template'];param=bytearray(hirc[a:b]);struct.pack_into('<I',param,0,ids[route['root']])
    if len(param)!=14:raise ValueError('Parámetros de cambio de sonido desconocidos.')
    inserts=[(route['param_insert']-start,bytes(param)),(route['child_insert']-start,struct.pack('<I',ids[route['root']]))]
    for at,payload in sorted(inserts,reverse=True):new[at:at]=payload
    struct.pack_into('<I',new,1,len(new)-5)
    serialized=[]
    for ident,_,serial in old:
        if ident==route['parent']:serialized.extend(clones);serialized.append(bytes(new))
        else:serialized.append(serial)
    updated_hirc=struct.pack('<I',len(old)+len(clones))+b''.join(serialized)
    # Verify original sound objects and every non-post switch remain byte exact.
    parsed={i:serial for i,_,serial in objects(updated_hirc)}
    if any(parsed[i]!=serial for i,_,serial in old if i!=route['parent']):raise ValueError('Cambió un sonido original.')
    wanted={r['offset'] for r in route['post_refs']}
    for switch in route['switches']:
        for ref in switch['refs']:
            at=ref['offset']-start+(4 if ref['offset']>=route['child_insert'] else 0)
            expected=ids[route['root']] if ref['offset'] in wanted else ref['value']
            if struct.unpack_from('<I',new,at)[0]!=expected:raise ValueError('Cambió otra superficie de pelota.')
    final_entries={i:(off,size) for i,off,size in entries}
    if bytes(data[:len(parts[b'DATA'])])!=parts[b'DATA'] or any(final_entries[i]!=entry for i,entry in original_entries.items()):raise ValueError('Cambió una muestra original.')
    updated={b'HIRC':updated_hirc,b'DIDX':bytes(index),b'DATA':bytes(data)}
    return pack_aligned(chunks,updated,parse_chunks)
