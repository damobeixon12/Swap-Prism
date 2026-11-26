"""Client presentation only. Offline file preparation; no process/network access."""
import struct
from hud_abc import ABC,Reader,abc_tag,encode_u30,swf_tags
from cosmetic_colors import export_properties
from modern_upk import Package,i32,i64,put32
from texture_engine import append_exports,inline_serial


def validate_name(value):
    if not isinstance(value,str):raise ValueError('El nombre debe ser texto.')
    value=value.strip()
    if len(value)>32 or any(ord(c)<32 or ord(c)==127 or 0x202a<=ord(c)<=0x202e or 0x2066<=ord(c)<=0x2069 for c in value):
        raise ValueError('Usa hasta 32 caracteres, sin controles ni saltos de línea.')
    return value


def _getter(abc,cls,static,field):
    record=abc.classes.get(cls,{})
    traits=record.get('static_traits' if static else 'traits',[])
    found=[t['method'] for t in traits if t['kind']==2 and t['name'].split('.')[-1]==field]
    if len(found)>1:raise ValueError('Getter de presentación ambiguo.')
    return found[0] if found else None


def _verified_getter(abc,mid,static,table='PlayerInfo'):
    """Accept only the linear DataAPI read from the exact PlayerName field."""
    b=abc.bodies[mid]
    if b['exceptions'] or abc.methods[mid]['params'] or abc.name(abc.methods[mid]['return'])!='String':
        raise ValueError('El getter de nombre cambió de firma.')
    r=Reader(b['code']);ops=[];ends=[]
    counts={0xd0:0,0x30:0,0x5d:1,0x4f:2,0x60:1,0x2c:1,0x24:1,0x66:1,0x46:2,0x48:0}
    while r.pos<len(r.data):
        op=r.byte()
        if op in (0xf0,0xf1):r.u30();continue # debugline / debugfile
        if op not in counts:raise ValueError('Getter de nombre desconocido '+hex(op)+' en '+str(mid)+'; no se modifica.')
        args=[r.byte() if op==0x24 else r.u30() for _ in range(counts[op])];ops.append((op,args));ends.append(r.pos)
    strings=[abc.strings[a[0]] for op,a in ops if op==0x2c]
    if strings!=['LocalPlayer' if static else table,'PlayerName'] or ops[0:2]!=[(0xd0,[]),(0x30,[])]:
        raise ValueError('Getter de nombre sin el origen esperado.')
    if [(abc.name(a[0]),a[1]) for op,a in ops if op==0x46]!=[('getValue',3)]:
        raise ValueError('El getter no es una lectura de DataAPI.')
    if not any(op==0x60 and abc.name(a[0])=='projectx.api.DataAPI' for op,a in ops):
        raise ValueError('No se reconoce DataAPI.')
    if ops[-1]!=(0x48,[]):raise ValueError('Retorno no reconocido.')
    expected=[0xd0,0x30,0x5d,0x4f,0x60,0x2c,0x24,0x2c,0x46,0x48] if static else [0xd0,0x30,0x60,0x2c,0xd0,0x66,0x2c,0x46,0x48]
    if [op for op,_ in ops]!=expected:raise ValueError('Getter de nombre fuera del perfil verificado.')
    if static and (abc.name(ops[2][1][0])!='init' or abc.name(ops[3][1][0])!='init' or ops[3][1][1]!=0):
        raise ValueError('Inicialización LocalPlayer desconocida.')
    return b,ends[1],ends[3] if static else None


def patch_abc(data,name):
    name=validate_name(name)
    if not name:return data,[]
    abc=ABC(data);patches=[];changed={};fields=[]
    sid=len(abc.strings)
    for cls,static in [('tagame.api.LocalPlayer',True),('tagame.api.PlayerInfoRow',False),('tagame.api.NameplateRow',False)]:
        mid=_getter(abc,cls,static,'PlayerName')
        if mid is None:continue
        body,scope_end,init_end=_verified_getter(abc,mid,static,'Nameplate' if cls.endswith('.NameplateRow') else 'PlayerInfo')
        override=b'\x2c'+encode_u30(sid)+b'\x48'
        if static:
            # Retain init() and its registration effects, replacing only the read.
            new=body['code'][:init_end]+override
        else:
            local=_getter(abc,cls,False,'bLocalPlayer')
            if local is None:raise ValueError('No se puede distinguir al jugador local.')
            # Read bLocalPlayer; other rows execute their exact original getter.
            ids=[i for i in range(1,len(abc.multinames)) if abc.name(i)=='bLocalPlayer']
            if len(ids)!=1:raise ValueError('Campo bLocalPlayer ambiguo.')
            check=b'\xd0\x66'+encode_u30(ids[0])+b'\x12'+len(override).to_bytes(3,'little')
            new=body['code'][:scope_end]+check+override+body['code'][scope_end:]
        changed[mid]=new;patches.append((body['lengthpos'],body['codepos']+len(body['code']),encode_u30(len(new))+new))
        fields.append(cls+'.PlayerName')
    # Goal messages receive a preformatted native string, bypassing PlayerName
    # getters. Replace our original local name; leave all other message text unchanged.
    record=abc.classes.get('tagame.hud.GameMessageView',{})
    goals=[t['method'] for t in record.get('traits',[]) if t['kind']==1 and t['name'].endswith('.handleGameMessageReceived')]
    # StartMenu also embeds this class as unused shared code, but omits String.replace.
    # All live HUD / replay / spectator / CodeLibrary consumers contain it.
    has_replace=any(abc.name(i)=='http://adobe.com/AS3/2006/builtin.replace' for i in range(1,len(abc.multinames)))
    if len(goals)==1 and has_replace:
        from title_display import _ref
        from avm2_read import instructions
        mid=goals[0];body=abc.bodies[mid];ops=[op for op in instructions(body['code']) if op[2] not in [0xf0,0xf1]]
        if body['exceptions'] or body['stack']<6 or [v[2] for v in ops[:2]]!=[0xd0,0x30]:raise ValueError('El renderizador del mensaje de gol cambió.')
        ref=lambda key:encode_u30(_ref(abc,key))
        # Native goal text can already be uppercased and arrives before replay
        # scorer replication. Read our own original name directly from DataAPI.
        table=abc.strings.index('LocalPlayer');field=abc.strings.index('PlayerName')
        original=b'\x60'+ref('projectx.api.DataAPI')+b'\x2c'+encode_u30(table)+b'\x24\x00\x2c'+encode_u30(field)+b'\x46'+ref('getValue')+b'\x03'
        upper=b'\x46'+ref('http://adobe.com/AS3/2006/builtin.toUpperCase')+b'\x00'
        prefix=bytearray();branches=[]
        prefix.extend(original);branches.append(len(prefix));prefix.extend(b'\x12\x00\x00\x00')
        prefix.extend(b'\xd1\xd1\x66'+ref('message')+upper+original+upper+b'\x2c'+encode_u30(sid)+b'\x46'+ref('http://adobe.com/AS3/2006/builtin.replace')+b'\x02\x61'+ref('message'))
        for at in branches:prefix[at+1:at+4]=(len(prefix)-at-4).to_bytes(3,'little')
        scope_end=ops[1][1];new=body['code'][:scope_end]+bytes(prefix)+body['code'][scope_end:]
        changed[mid]=new;patches.append((body['lengthpos'],body['codepos']+len(body['code']),encode_u30(len(new))+new));fields.append('GameMessageView.localGoalName')
    if not changed:return data,[]
    raw=name.encode('utf8')
    patches.append((abc.string_pool_end,abc.string_pool_end,encode_u30(len(raw))+raw))
    r=Reader(data);r.pos=abc.string_count_pos;r.u30()
    patches.append((abc.string_count_pos,r.pos,encode_u30(len(abc.strings)+1)))
    result=data
    for start,end,replacement in sorted(patches,reverse=True):result=result[:start]+replacement+result[end:]
    checked=ABC(result)
    if checked.strings[:-1]!=abc.strings or checked.strings[-1]!=name:raise ValueError('Cambió otra constante.')
    if checked.classes!=abc.classes or checked.methods!=abc.methods:raise ValueError('Cambió la definición de clases.')
    for mid,body in abc.bodies.items():
        new=checked.bodies[mid]
        if new['code']!=changed.get(mid,body['code']):raise ValueError('Cambió otro método.')
        if any(new[k]!=body[k] for k in ['stack','locals','initial_scope','max_scope','exceptions']):raise ValueError('Cambió una firma de método.')
    return result,fields


def patch_movie(movie,name,title=None,avatar=None,rank=None):
    from avatar_display import add_local_frame,patch_avatar_abc
    avatar_fields=[]
    if avatar:
        movie,embedded=add_local_frame(movie,avatar)
        if embedded:avatar_fields.append('TintedAvatarIcons.localFrame')
    start,body,end,pos,abc=abc_tag(movie)
    from title_display import patch_title_abc
    new,title_fields=patch_title_abc(movie[pos:end],title)
    new,fields=patch_abc(new,name)
    features={'name'} if fields else set()
    if title_fields:features.add('title')
    fields+=title_fields
    if rank:
        from rank_display import patch_rank_abc
        new,rank_fields=patch_rank_abc(new,rank);fields+=rank_fields
        if rank_fields:features.add('rank')
    if avatar:
        new,scoped=patch_avatar_abc(new);fields+=scoped
        if scoped or avatar_fields:features.add('avatar')
    fields+=avatar_fields
    if not fields:return movie,[],features
    payload=movie[body:pos]+new
    tag=struct.pack('<HI',(82<<6)|63,len(payload))+payload
    result=bytearray(movie[:start]+tag+movie[end:]);struct.pack_into('<I',result,4,len(result))
    abc_tag(bytes(result))
    return bytes(result),fields,features


def is_banner_item(pkg):
    return any(pkg.class_name(e)=='ProductAsset_PlayerBanner_TA' for e in pkg.exports)


def patch_package(pkg,name='',images=None,title=None,avatar=None,rank=None):
    """Return package and exact changed exports; never installs it."""
    name=validate_name(name);flat=pkg.flat();replacements={};movies=set();fields=[];paths=[];features=set()
    avatar=avatar or (images or {}).get('avatar')
    # The goal replay HUD flags the replayed player as local, which tinted other players' photos black.
    if pkg.file.name.casefold()=='gfx_replayhud_sf.upk':avatar=None
    if name or title or avatar or rank:
        for i,e in enumerate(pkg.exports):
            if pkg.class_name(e)!='SwfMovie':continue
            data,props=export_properties(pkg,flat,e)
            raw=next((p for p in props if p['name']=='RawData' and p['kind']=='ArrayProperty'),None)
            if not raw:continue
            if i32(data,raw['value'])!=raw['size']-4:raise ValueError('RawData no válido.')
            at=raw['value']+4;movie=data[at:at+raw['size']-4]
            # Some static GFX have no ActionScript; leave them intact.
            if not any(code==82 for code,*_ in swf_tags(movie)):continue
            new,changed,found=patch_movie(movie,name,title,avatar,rank);features|=found
            if not changed:continue
            serial=bytearray(data[:at]+new+data[at+len(movie):])
            put32(serial,raw['value']-8,4+len(new));put32(serial,raw['value'],len(new))
            check=next((p for p in props if p['name'] in ('RawDataCRC','SourceFileCRC') and p['size']==4),None)
            if check:
                import zlib
                struct.pack_into('<I',serial,check['value']+(len(new)-len(movie) if check['value']>at else 0),zlib.crc32(new))
            replacements[i]=bytes(serial);movies.add(i);fields+=changed;paths.append(pkg.path(i))
    allowed={'avatar':'gfx_shared.Icon_AvatarDefault','banner':'gfx_shared.PlayerBanner_Default'}
    for key,raw in (images or {}).items():
        if key=='avatar':continue # Embedded local frame; never replace the shared fallback.
        # Startup also holds a banner product asset: there the shared default banner is the target, not its largest texture.
        if key=='banner' and is_banner_item(pkg) and pkg.file.name.casefold()!='startup.upk':
            # The equipped banner item shows its own artwork, not the shared default: replace its main texture.
            art=[i for i,e in enumerate(pkg.exports) if pkg.class_name(e)=='Texture2D' and not pkg.path(i).rsplit('.',1)[-1].startswith('Thumb_')]
            if not art:raise ValueError('El banner equipado no tiene una textura reconocible.')
            found=[max(art,key=lambda i:i32(pkg.exports[i],32))]
        else:
            if pkg.file.name.casefold()!='startup.upk' or key not in allowed:raise ValueError('Textura de perfil no permitida.')
            found=[i for i,e in enumerate(pkg.exports) if pkg.class_name(e)=='Texture2D' and pkg.path(i)==allowed[key]]
        if len(found)!=1:raise ValueError('No se localiza la textura exacta del perfil.')
        i=found[0];e=pkg.exports[i];serial=flat[i64(e,36):i64(e,36)+i32(e,32)]
        replacements[i]=inline_serial(pkg,serial,raw,fit=True);paths.append(pkg.path(i));features.add(key)
    if not replacements:return None,{}
    result=append_exports(pkg,replacements,swf_validator=lambda i,serial:i in movies and serial==replacements[i])
    return result,{'only_client_presentation':True,'other_payloads_unchanged':True,'fields':sorted(set(fields)),'exports':paths,'features':sorted(features),'visual_verified':False}
