"""Pin only the start-screen logo to an embedded local texture.

Does not touch login, input, legal credits or any game/account data API.
"""
import struct
from hud_abc import ABC,Reader,encode_u30,abc_tag
from avm2_read import instructions

# projectx.controls.ImageLoader.image prepends img:// when loading a texture.
PATH='UI_MainMenu_Logo.Event_Logo_RadSummer'

def patch_abc(data):
    abc=ABC(data);sid=len(abc.strings);patches=[];changed={};hits=[]
    owner=abc.classes.get('menus.StartMenu',{})
    handlers={t['method'] for t in owner.get('traits',[]) if t['kind']==1 and t['name'].endswith('.onDynamicLogoLoaded')}
    for mid,body in abc.bodies.items():
        code=body['code'];ops=list(instructions(code));new=bytearray(code)
        if mid in handlers:
            # The native online callback can arrive before START is active. Our
            # embedded image is ready synchronously; do not lose that first draw.
            gates=[op for op in ops if op[2]==0x12 and op[0]<34]
            if len(gates)!=2 or gates[-1][1]-gates[-1][0]!=4:raise ValueError('La condición visual del logo cambió.')
            at,end,_,_=gates[-1];new[at:end]=b'\x29\x02\x02\x02';hits.append('embedded_logo_ready')
        for i,op in enumerate(ops[:-1]):
            at,end,opcode,args=op;next_at,next_end,next_opcode,next_args=ops[i+1]
            if opcode!=0x60 or abc.name(args[0])!='tagame.api.DynamicLogos' or next_opcode!=0x46:continue
            method=abc.name(next_args[0])
            if next_args[1]!=0 or method not in ['GetLogoImage','IsDynamicLogoLoaded']:continue
            replacement=(b'\x2c'+encode_u30(sid)) if method=='GetLogoImage' else b'\x26'
            if len(replacement)>next_end-at:raise ValueError('La instrucción de logo no cabe.')
            new[at:next_end]=replacement+b'\x02'*(next_end-at-len(replacement));hits.append(method)
        if new!=code:
            changed[mid]=bytes(new);patches.append((body['codepos'],body['codepos']+len(code),bytes(new)))
    if 'GetLogoImage' not in hits or 'IsDynamicLogoLoaded' not in hits:raise ValueError('No se reconoce el renderizador de inicio.')
    raw=PATH.encode();patches.append((abc.string_pool_end,abc.string_pool_end,encode_u30(len(raw))+raw))
    r=Reader(data);r.pos=abc.string_count_pos;r.u30();patches.append((abc.string_count_pos,r.pos,encode_u30(len(abc.strings)+1)))
    result=data
    for start,end,new in sorted(patches,reverse=True):result=result[:start]+new+result[end:]
    checked=ABC(result)
    if checked.classes!=abc.classes or checked.methods!=abc.methods or checked.strings[:-1]!=abc.strings:raise ValueError('Cambió una definición ajena al logo.')
    for mid,body in abc.bodies.items():
        new=checked.bodies[mid]
        if new['code']!=changed.get(mid,body['code']) or any(new[k]!=body[k] for k in ['stack','locals','initial_scope','max_scope','exceptions']):raise ValueError('Cambio fuera de las lecturas de logo.')
    return result,{'only_start_menu_logo_presentation':True,'methods':len(changed),'reads':hits}

def patch_movie(movie):
    start,body,end,pos,abc=abc_tag(movie);new,proof=patch_abc(movie[pos:end]);payload=movie[body:pos]+new
    tag=struct.pack('<HI',(82<<6)|63,len(payload))+payload
    result=bytearray(movie[:start]+tag+movie[end:]);struct.pack_into('<I',result,4,len(result));abc_tag(bytes(result));return bytes(result),proof
