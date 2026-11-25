"""Bounded title presentation edits. Never grants/equips account titles."""
import html,re
from hud_abc import ABC,Reader,encode_u30
from fake_display import _getter

def normalize_title(value):
    if not value or value.get('enabled') is False:return None
    text=value.get('text','').strip()
    if not text or len(text)>48 or any(ord(c)<32 or ord(c)==127 or 0x202a<=ord(c)<=0x202e or 0x2066<=ord(c)<=0x2069 for c in text):
        raise ValueError('El título necesita entre 1 y 48 caracteres sin controles.')
    target=value.get('target','')
    if not isinstance(target,str) or not target or len(target)>128:raise ValueError('Elige un título original de tu inventario.')
    out={'enabled':True,'text':text,'target':target,'multicolor':bool(value.get('multicolor',False))}
    for key,default in [('color','#75EAA6'),('color2','#68DAF9'),('glow','#75EAA6')]:
        color=value.get(key,default)
        if not isinstance(color,str) or not re.fullmatch(r'#[0-9a-fA-F]{6}',color):raise ValueError('Color de título no válido.')
        out[key]=color.upper()
    out['glow_enabled']=bool(value.get('glow_enabled',True))
    return out

def title_text(title):
    text=title['text'].upper()
    if not title['multicolor']:return text
    a=bytes.fromhex(title['color'][1:]);b=bytes.fromhex(title['color2'][1:]);parts=[]
    for i,c in enumerate(text):
        t=i/max(1,len(text)-1);rgb=''.join(f'{round(x+(y-x)*t):02X}' for x,y in zip(a,b))
        escaped={'&':'&#38;','<':'&#60;','>':'&#62;','"':'&#34;',"'":'&#39;'}.get(c,c)
        parts.append(f'<FONT COLOR="#{rgb}">{escaped}</FONT>')
    return ''.join(parts)

def _ref(abc,name):
    if not hasattr(abc,'_title_name_map'):
        abc._title_name_map={}
        for i in range(1,len(abc.multinames)):abc._title_name_map.setdefault(abc.name(i),[]).append(i)
    found=abc._title_name_map.get(name,[])
    public=[i for i in found if abc.multinames[i][0] in (7,13) and abc.namespaces[abc.multinames[i][1]][0]==0x16 and abc.strings[abc.namespaces[abc.multinames[i][1]][1]]=='']
    if public:return public[0]
    sets=[i for i in found if abc.multinames[i][0] in (9,14) and any(abc.namespaces[n][0]==0x16 and abc.strings[abc.namespaces[n][1]]=='' for n in abc.nssets[abc.multinames[i][1]])]
    if sets:return min(sets,key=lambda i:len(abc.nssets[abc.multinames[i][1]]))
    if len(found)!=1:raise ValueError('Referencia de título ambigua: '+name)
    return found[0]

def _read(abc,mid,table,field,return_type):
    body=abc.bodies[mid];r=Reader(body['code']);ops=[];ends=[]
    if body['exceptions'] or abc.methods[mid]['params'] or abc.name(abc.methods[mid]['return'])!=return_type:raise ValueError('Firma de título no reconocida.')
    counts={0xd0:0,0x30:0,0x60:1,0x2c:1,0x66:1,0x46:2,0x48:0}
    while r.pos<len(r.data):
        op=r.byte()
        if op in (0xf0,0xf1):r.u30();continue
        if op not in counts:raise ValueError('Getter de título fuera del perfil verificado.')
        args=[r.u30() for _ in range(counts[op])];ops.append((op,args));ends.append(r.pos)
    if [op for op,_ in ops]!=[0xd0,0x30,0x60,0x2c,0xd0,0x66,0x2c,0x46,0x48]:raise ValueError('Estructura de título desconocida.')
    if [abc.strings[a[0]] for op,a in ops if op==0x2c]!=[table,field] or abc.name(ops[2][1][0])!='projectx.api.DataAPI' or abc.name(ops[-2][1][0])!='getValue' or ops[-2][1][1]!=3:
        raise ValueError('Origen de título desconocido.')
    if abc.name(ops[5][1][0])!='RowIndex':raise ValueError('Índice de título no reconocido.')
    return body,ends[1]

def patch_title_abc(data,value):
    title=normalize_title(value)
    if not title:return data,[]
    abc=ABC(data);edits=[];changed={};fields=[];strings=[]
    def string(value):
        if value not in strings:strings.append(value)
        return b'\x2c'+encode_u30(len(abc.strings)+strings.index(value))
    def prop(name):return b'\x66'+encode_u30(_ref(abc,name))
    def lex(name):return b'\x60'+encode_u30(_ref(abc,name))
    colors={'Text':title_text(title),'Color':str(0xffffff if title['multicolor'] else int(title['color'][1:],16)),
            'GlowColor':str(int(title['glow'][1:],16) if title['glow_enabled'] else 0)}
    cls='tagame.api.PlayerTitlesPlayerTitlesRow'
    for field in ['Text','Color','GlowColor']:
        mid=_getter(abc,cls,False,field)
        if mid is None:continue
        body,scope=_read(abc,mid,'PlayerTitlesPlayerTitles',field,'String' if field=='Text' else 'uint')
        override=string(colors[field])+(b'\x74' if field!='Text' else b'')+b'\x48'
        check=b'\xd0'+prop('RowIndex')+lex('tagame.api.PlayerTitles')+prop('SelectedTitle')+b'\x14'+len(override).to_bytes(3,'little')
        changed[mid]=body['code'][:scope]+check+override+body['code'][scope:];fields.append(cls+'.'+field)
    # Per-player title display is guarded by bLocalPlayer and the original title's
    # localized text, retrieved from the game's existing read-only title API.
    cls='tagame.api.PlayerInfoRow';xp=_getter(abc,cls,False,'XPTitle')
    if xp is not None:
        raw,raw_scope=_read(abc,xp,'PlayerInfo','XPTitle','String')
        native_read=raw['code'][raw_scope:-1]
        for field,key in [('XPTitle','Text'),('TitleColor','Color'),('TitleGlowColor','GlowColor')]:
            mid=_getter(abc,cls,False,field)
            if mid is None:continue
            body,scope=_read(abc,mid,'PlayerInfo',field,'String' if key=='Text' else 'uint')
            # PlayerInfo is consumed by plain-text HUD fields as well as menus.
            # Keep markup out of this shared API; render HTML only at a local view.
            value=title['text'].upper() if key=='Text' else colors[key]
            override=string(value)+(b'\x74' if key!='Text' else b'')+b'\x48'
            check=b'\xd0'+prop('bLocalPlayer')+b'\x12'+len(override).to_bytes(3,'little')
            changed[mid]=body['code'][:scope]+check+override+body['code'][scope:];fields.append(cls+'.'+field)
    cls='tagame.controls.LocalPlayerView'
    for trait in abc.classes.get(cls,{}).get('traits',[]):
        if trait['kind']!=1 or trait['name'].split('.')[-1]!='updatePlayerTitle':continue
        mid=trait['method'];body=abc.bodies[mid];r=Reader(body['code'])
        while r.data[r.pos] in (0xf0,0xf1):r.byte();r.u30()
        if r.byte()!=0xd0 or r.byte()!=0x30 or body['exceptions'] or body['stack']<4:raise ValueError('Renderizador de título no reconocido.')
        scope=r.pos
        override=lex('tagame.managers.PlayerTitleStyleManager')+b'\x46'+encode_u30(_ref(abc,'getInstance'))+b'\x00'
        field=prop('tagame.controls:LocalPlayerView.rankNameTextField')
        override+=b'\xd0'+field+string(colors['Color'])+b'\x74'+string(colors['GlowColor'])+b'\x74\x4f'+encode_u30(_ref(abc,'applyTitleStyle'))+b'\x03'
        override+=b'\xd0'+field+string(colors['Text'])+b'\x61'+encode_u30(_ref(abc,'htmlText' if title['multicolor'] else 'text'))+b'\x47'
        preview='tagame.controls:LocalPlayerView.m_playerTitlePreviewRow'
        has_preview=any(t['name']==preview for t in abc.classes[cls]['traits'])
        guard=b'\xd0'+prop(preview)+b'\x11'+len(override).to_bytes(3,'little') if has_preview else b''
        changed[mid]=body['code'][:scope]+guard+override+body['code'][scope:];fields.append(cls+'.equippedTitle')
    cls='tagame.controls.ScoreboardRenderer'
    for trait in abc.classes.get(cls,{}).get('traits',[]):
        if trait['kind']!=1 or trait['name'].split('.')[-1]!='setPlayerTitle':continue
        mid=trait['method'];body=abc.bodies[mid]
        reader=Reader(body['code'])
        while reader.data[reader.pos] in (0xf0,0xf1):reader.byte();reader.u30()
        if body['exceptions'] or reader.byte()!=0xd0 or reader.byte()!=0x30 or body['code'][-1:]!=b'\x47' or [abc.name(x) for x in abc.methods[mid]['params']]!=['String']:
            raise ValueError('El renderizador del título del marcador cambió.')
        scope=reader.pos
        row=b'\xd0'+prop('m_playerData')
        def local_only(code):
            inner=row+prop('bLocalPlayer')+b'\x12'+len(code).to_bytes(3,'little')+code
            return row+b'\x12'+len(inner).to_bytes(3,'little')+inner
        # Let native layout/style run with plain text. Apply HTML afterwards so
        # native applyTitleStyle cannot flatten the individual gradient colors.
        before=local_only(string(title['text'].upper())+b'\x85\xd5')
        after=local_only(b'\xd0'+prop('xpRankTextField')+string(colors['Text'])+b'\x61'+encode_u30(_ref(abc,'htmlText')))
        changed[mid]=body['code'][:scope]+before+body['code'][scope:-1]+after+b'\x47'
        fields.append(cls+'.localTitleHtml')
    # Only known title rendering methods switch text -> htmlText for the generated
    # FONT runs. Operands are padded; original jump/exception offsets never move.
    if title['multicolor'] and any(c in abc.classes for c in ['tagame.controls.LocalPlayerView','tagame.controls.TitleListItemRenderer']):
        # Locate literal setproperty instructions by a full instruction decoder.
        from avm2_read import instructions
        html_id=_ref(abc,'htmlText')
        for cls,methods in []:
            for trait in abc.classes.get(cls,{}).get('traits',[]):
                if trait['kind']!=1 or trait['name'].split('.')[-1] not in methods:continue
                mid=trait['method'];body=abc.bodies[mid];code=bytearray(body['code']);sites=0
                for start,end,op,args in instructions(bytes(code)):
                    if op==0x61 and abc.name(args[0])=='text':
                        repl=b'\x61'+encode_u30(html_id)
                        if len(repl)>end-start:raise ValueError('El renderer no admite multicolor sin mover saltos.')
                        code[start:end]=repl+b'\x02'*(end-start-len(repl));sites+=1
                if sites:changed[mid]=bytes(code);fields.append(cls+'.htmlText')
        cls='tagame.controls.TitleListItemRenderer'
        for trait in abc.classes.get(cls,{}).get('traits',[]):
            if trait['kind']!=1 or trait['name'].split('.')[-1]!='setData':continue
            mid=trait['method'];body=abc.bodies[mid]
            if body['exceptions'] or body['code'][-1:]!=b'\x47':raise ValueError('El renderer de lista ha cambiado.')
            row=prop('tagame.controls:TitleListItemRenderer.m_titleRow')
            render=lex('textField')+b'\xd0'+row+prop('Text')+b'\x46'+encode_u30(_ref(abc,'toUpperCase'))+b'\x00'+b'\x61'+encode_u30(html_id)
            check=b'\xd0'+row+b'\x12'+len(render).to_bytes(3,'little')
            changed[mid]=body['code'][:-1]+check+render+b'\x47';fields.append(cls+'.htmlText')
    if not changed:return data,[]
    for mid,code in changed.items():
        b=abc.bodies[mid];edits.append((b['lengthpos'],b['codepos']+len(b['code']),encode_u30(len(code))+code))
    payload=b''.join(encode_u30(len(s.encode('utf8')))+s.encode('utf8') for s in strings)
    edits.append((abc.string_pool_end,abc.string_pool_end,payload))
    r=Reader(data);r.pos=abc.string_count_pos;r.u30();edits.append((abc.string_count_pos,r.pos,encode_u30(len(abc.strings)+len(strings))))
    result=data
    for start,end,repl in sorted(edits,reverse=True):result=result[:start]+repl+result[end:]
    after=ABC(result)
    if after.strings[:len(abc.strings)]!=abc.strings or after.classes!=abc.classes or after.methods!=abc.methods:raise ValueError('Cambió una definición fuera del título.')
    for mid,b in abc.bodies.items():
        if after.bodies[mid]['code']!=changed.get(mid,b['code']):raise ValueError('Cambió otro método al preparar títulos.')
    return result,fields
