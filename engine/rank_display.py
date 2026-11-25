"""Local ranked-menu getters only. Never writes account skill or matchmaking data."""
from hud_abc import ABC,Reader,encode_u30
from avm2_read import instructions
from title_display import _ref

PLAYLISTS={10,11,13,27,28,29,30}

def normalize_rank(value):
    if value is not None and not isinstance(value,dict):raise ValueError('Configuración de rango no válida.')
    if not value or value.get('enabled') is False:return None
    result={'enabled':True}
    for name,low,high in [('playlist',0,100),('mmr',0,10000),('tier',0,22),('division',0,3)]:
        n=value.get(name)
        if type(n) is not int or not low<=n<=high:raise ValueError('Rango visual no válido: '+name)
        result[name]=n
    if result['playlist'] not in PLAYLISTS:raise ValueError('Modalidad de rango no disponible.')
    return result

def patch_rank_abc(data,value):
    rank=normalize_rank(value)
    if not rank:return data,[]
    abc=ABC(data);changes={};fields=[];edits=[]
    override={'SkillTier':rank['tier'],'PrevSkillTier':rank['tier'],'SkillDivision':rank['division'],'PrevSkillDivision':rank['division'],'SkillMMR':rank['mmr'],'bCompletedPlacementMatches':rank['tier']!=0}
    for cls,static in [('tagame.api.Playlists',True),('tagame.api.PlaylistsRow',False)]:
        record=abc.classes.get(cls,{})
        for trait in record.get('static_traits' if static else 'traits',[]):
            name=trait['name'].split('.')[-1];field=name[3:] if static and name.startswith('Get') else name
            if field not in override or trait['kind']!=(1 if static else 2):continue
            mid=trait['method'];body=abc.bodies[mid];method=abc.methods[mid]
            expected_return='Boolean' if field.startswith('b') else 'Number' if field=='SkillMMR' else 'int'
            if body['exceptions'] or [abc.name(p) for p in method['params']]!=(['int'] if static else []) or abc.name(method['return'])!=expected_return:raise ValueError('Firma de rango modificada; no se parchea.')
            # Debug metadata differs in PlayMenuV4. Preserve it byte for byte,
            # but exclude it from the executable getter shape validation.
            ops=[v for v in instructions(body['code']) if v[2] not in [0xef,0xf0,0xf1]]
            expected=[0xd0,0x30,0x60,0x2c,0xd1,0x2c,0x46,0x48] if static else [0xd0,0x30,0x60,0x2c,0xd0,0x66,0x2c,0x46,0x48]
            if [v[2] for v in ops]!=expected:raise ValueError('Getter de rango fuera del perfil verificado: '+cls+'.'+name+' '+','.join(hex(v[2]) for v in ops))
            if [abc.strings[v[3][0]] for v in ops if v[2]==0x2c]!=['Playlists',field] or abc.name(ops[2][3][0])!='projectx.api.DataAPI' or abc.name(ops[-2][3][0])!='getValue' or ops[-2][3][1]!=3:raise ValueError('El getter no es una lectura de presentación de Playlists.')
            if not static and abc.name(ops[5][3][0])!='RowIndex':raise ValueError('Índice de fila desconocido.')
            # Match the native playlist Id, not its incidental row position.
            row=b'\xd1' if static else b'\xd0\x66'+encode_u30(_ref(abc,'RowIndex'))
            guard=b'\x60'+encode_u30(_ref(abc,'projectx.api.DataAPI'))+b'\x2c'+encode_u30(abc.strings.index('Playlists'))+row+b'\x2c'+encode_u30(abc.strings.index('Id'))+b'\x46'+encode_u30(_ref(abc,'getValue'))+b'\x03'
            guard+=b'\x25'+encode_u30(rank['playlist'])
            val=override[field]
            replacement=(b'\x26' if val else b'\x27') if type(val) is bool else b'\x25'+encode_u30(val)
            if field=='SkillMMR':
                # Rankings.convertMMR displays round(native * 20 + 100).
                # Return its inverse, preserving fractional ratings and values <100.
                replacement+=b'\x25'+encode_u30(100)+b'\xa1\x25'+encode_u30(20)+b'\xa3'
            replacement+=b'\x48'
            prefix=guard+b'\x14'+len(replacement).to_bytes(3,'little')+replacement
            end=ops[1][1];code=body['code'][:end]+prefix+body['code'][end:]
            if body['stack']<4:raise ValueError('Pila de rango no reconocida.')
            changes[mid]=code;fields.append(cls+'.'+name)
            edits.append((body['lengthpos'],body['codepos']+len(body['code']),encode_u30(len(code))+code))
    if not changes:return data,[]
    result=data
    for a,b,raw in sorted(edits,reverse=True):result=result[:a]+raw+result[b:]
    checked=ABC(result)
    if checked.methods!=abc.methods or checked.classes!=abc.classes or checked.strings!=abc.strings:raise ValueError('Cambió la definición de rango.')
    for mid,body in abc.bodies.items():
        after=checked.bodies[mid]
        if after['code']!=changes.get(mid,body['code']):raise ValueError('Cambió un método ajeno al rango.')
        if any(after[k]!=body[k] for k in ['stack','locals','initial_scope','max_scope','exceptions']):raise ValueError('Cambió el contrato del getter.')
        if mid in changes:instructions(after['code'])
    return result,fields
