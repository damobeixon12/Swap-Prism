"""Bounded reader for the HUD's AVM2 data; never executes ActionScript."""
import struct

class Reader:
    def __init__(self,data):self.data=data;self.pos=0
    def byte(self):
        if self.pos>=len(self.data):raise ValueError('ABC truncado.')
        value=self.data[self.pos];self.pos+=1;return value
    def u30(self):
        value=0
        for n in range(5):
            part=self.byte();value|=(part&127)<<(7*n)
            if part<128:return value
        raise ValueError('Entero ABC inválido.')
    def skip(self,n):
        self.pos+=n
        if self.pos>len(self.data):raise ValueError('ABC fuera de rango.')

def encode_u30(value):
    out=bytearray()
    while value>=128:out.append((value&127)|128);value>>=7
    out.append(value);return bytes(out)

class ABC:
    def __init__(self,data):
        self.data=data;r=Reader(data);r.skip(4);self.ints=[0];self.uints=[0];self.doubles=[0]
        for _ in range(max(0,r.u30()-1)):self.ints.append(r.u30())
        for _ in range(max(0,r.u30()-1)):self.uints.append(r.u30())
        for _ in range(max(0,r.u30()-1)):self.doubles.append(struct.unpack_from('<d',data,r.pos)[0]);r.skip(8)
        self.string_count_pos=r.pos
        self.strings=['']
        for _ in range(max(0,r.u30()-1)):
            n=r.u30();self.strings.append(data[r.pos:r.pos+n].decode('utf-8'));r.skip(n)
        self.string_pool_end=r.pos
        self.namespaces=[(0,0)]
        for _ in range(max(0,r.u30()-1)):self.namespaces.append((r.byte(),r.u30()))
        self.nssets=[[]]
        for _ in range(max(0,r.u30()-1)):self.nssets.append([r.u30() for _ in range(r.u30())])
        self.multinames=[(0,0,0)]
        for _ in range(max(0,r.u30()-1)):
            kind=r.byte()
            if kind in (7,13):self.multinames.append((kind,r.u30(),r.u30()))
            elif kind in (15,16):self.multinames.append((kind,0,r.u30()))
            elif kind in (17,18):self.multinames.append((kind,0,0))
            elif kind in (9,14):
                name=r.u30();nsset=r.u30();self.multinames.append((kind,nsset,name))
            elif kind in (27,28):self.multinames.append((kind,r.u30(),0))
            elif kind==29:
                base=r.u30();params=[r.u30() for _ in range(r.u30())];self.multinames.append((kind,base,params))
            else:raise ValueError('Multiname ABC no reconocido: '+str(kind))
        self.methods=[]
        for _ in range(r.u30()):
            count=r.u30();ret=r.u30();params=[r.u30() for _ in range(count)];name=r.u30();flags=r.byte()
            if flags&8:
                for _ in range(r.u30()):r.u30();r.byte()
            if flags&128:
                for _ in range(count):r.u30()
            self.methods.append({'params':params,'return':ret,'name':name,'flags':flags})
        for _ in range(r.u30()):
            r.u30();count=r.u30()
            for _ in range(count*2):r.u30()
        count=r.u30();self.classes={}
        for _ in range(count):
            name=r.u30();r.u30();flags=r.byte()
            if flags&8:r.u30()
            for _ in range(r.u30()):r.u30()
            init=r.u30();traits=self.traits(r);self.classes[self.name(name)]={'init':init,'traits':traits}
        for cls in self.classes.values():
            cls['cinit']=r.u30();cls['static_traits']=self.traits(r)
        for _ in range(r.u30()):r.u30();self.traits(r)
        self.bodies={}
        for _ in range(r.u30()):
            start=r.pos;method=r.u30();stackpos=r.pos;stack=r.u30();locals=r.u30();initial=r.u30();scope=r.u30();lengthpos=r.pos;length=r.u30();codepos=r.pos;r.skip(length)
            exceptions=r.u30()
            for _ in range(exceptions*5):r.u30()
            self.traits(r)
            self.bodies[method]={'start':start,'end':r.pos,'stackpos':stackpos,'stack':stack,'locals':locals,'initial_scope':initial,
                'max_scope':scope,'lengthpos':lengthpos,'codepos':codepos,'code':data[codepos:codepos+length],'exceptions':exceptions}
        if r.pos!=len(data):raise ValueError('Hay datos ABC sin interpretar.')

    def name(self,index):
        if not index:return '*'
        kind,ns,name=self.multinames[index]
        if kind==29:return self.name(ns)+'<'+','.join(self.name(i) for i in name)+'>'
        text=self.strings[name]
        if kind in (7,13):
            namespace=self.strings[self.namespaces[ns][1]]
            if namespace:return namespace+'.'+text
        return text

    def traits(self,r):
        out=[]
        for _ in range(r.u30()):
            name=r.u30();flags=r.byte();kind=flags&15;value=None
            if kind in (0,6):
                r.u30();r.u30();v=r.u30()
                if v:r.byte()
            elif kind in (1,2,3,4,5):r.u30();value=r.u30()
            else:raise ValueError('Trait ABC no reconocido.')
            if flags&64:
                for _ in range(r.u30()):r.u30()
            out.append({'name':self.name(name),'kind':kind,'method':value})
        return out

def swf_tags(data):
    if data[:3]!=b'GFX':raise ValueError('El HUD no usa el formato GFX esperado.')
    n=data[8]>>3;p=8+(5+4*n+7)//8+4
    while p+2<=len(data):
        head=struct.unpack_from('<H',data,p)[0];code=head>>6;size=head&63;header=2
        if size==63:size=struct.unpack_from('<I',data,p+2)[0];header=6
        body=p+header;end=body+size
        if end>len(data):raise ValueError('Tag GFX fuera de rango.')
        yield code,p,body,end
        if code==0:break
        p=end

def abc_tag(data):
    matches=[(start,body,end) for code,start,body,end in swf_tags(data) if code==82]
    if len(matches)!=1:raise ValueError('No hay un único bloque ABC en el HUD.')
    start,body,end=matches[0];pos=data.index(b'\0',body+4,end)+1
    return start,body,end,pos,ABC(data[pos:end])

def decode_color_instructions(code):
    """Only the instruction subset present in BoostMeterView's display-colour method."""
    counts={0x08:1,0x20:0,0x29:0,0x2a:0,0x2d:1,0x2f:1,0x30:0,0x46:2,0x47:0,0x60:1,0x61:1,
            0x26:0,0x27:0,0x2c:1,0x4f:2,0x55:1,0x56:1,0x5d:1,0x66:1,0x68:1,0x74:0,0x75:0,
            0x80:1,0x82:0,0x85:0,0xa0:0,0xa3:0,0xa6:0,0xaf:0,0xb0:0,
            0xd0:0,0xd1:0,0xd2:0,0xd3:0,0xd5:0,0xd6:0,0xd7:0,0x02:0}
    reader=Reader(code);result=[]
    while reader.pos<len(code):
        at=reader.pos;op=reader.byte()
        if 0x0c<=op<=0x1a:reader.skip(3);args=[]
        elif op==0x24:args=[reader.byte()]
        elif op in counts:args=[reader.u30() for _ in range(counts[op])]
        else:raise ValueError('El método de color del HUD usa instrucciones no verificadas.')
        result.append((at,reader.pos,op,args))
    return result

def neutralize_color_method(code,name,identity_id=None,require_full_profile=True):
    instructions=decode_color_instructions(code)
    identity=[args[0] for _,_,op,args in instructions if op==0x60 and name(args[0])=='COLOR_NORMAL']
    if identity_id is None:
        if len(set(identity))!=1:raise ValueError('No se identifica la transformación neutra del boostmeter.')
        identity_id=identity[0]
    if name(identity_id)!='COLOR_NORMAL':raise ValueError('Constante neutra no válida.')
    neutral=b'\x60'+encode_u30(identity_id);out=bytearray(code);edits=[]
    replacements={'COLOR_GLOW_RED','COLOR_GLOW_AMBER','COLOR_BOOST_MAX','COLOR_BOOST_NORMAL','COLOR_BACKGROUND_AMBER'}
    found=set()
    for start,end,op,args in instructions:
        if op==0x60 and name(args[0]) in replacements:
            if len(neutral)>end-start:raise ValueError('El reemplazo de color cambia los saltos del HUD.')
            out[start:end]=neutral+b'\x02'*(end-start-len(neutral));edits.append((start,end));found.add(name(args[0]))
    team=0
    for a,b,c in zip(instructions,instructions[1:],instructions[2:]):
        if a[2]==0xd0 and b[2]==0x66 and name(b[3][0]).endswith('.m_teamStyle') and c[2]==0x66 and name(c[3][0])=='colorTransform':
            start,end=a[0],c[1];out[start:end]=neutral+b'\x02'*(end-start-len(neutral));edits.append((start,end));team+=1
    if require_full_profile and (found!=replacements or team!=1):raise ValueError('El HUD no coincide con el perfil de colores verificado.')
    if not edits:raise ValueError('No se localizaron colores de presentación en el método del HUD.')
    allowed={i for a,b in edits for i in range(a,b)}
    if any(a!=b and i not in allowed for i,(a,b) in enumerate(zip(code,out))):raise ValueError('Cambio fuera del color del boostmeter.')
    return bytes(out),edits

def neutral_boostmeter_gfx(data):
    _,_,_,start,abc=abc_tag(data)
    cls=abc.classes.get('tagame.hud.BoostMeterView')
    if not cls:raise ValueError('No se identifica BoostMeterView.')
    matches=[t for t in cls['traits'] if t['kind']==1 and t['name'].endswith('.onTeamColorChanged')]
    if len(matches)!=1:raise ValueError('No hay un único método de color del boostmeter.')
    method=matches[0]['method'];body=abc.bodies[method]
    if abc.methods[method]['params'] or body['exceptions']:raise ValueError('El método de color del HUD cambió de estructura.')
    instructions=decode_color_instructions(body['code'])
    identity=next(args[0] for _,_,op,args in instructions if op==0x60 and abc.name(args[0])=='COLOR_NORMAL')
    out=bytearray(data);changed={};allowed_bytes=set();sites=0
    names=('onTeamColorChanged','onBoostChanged','updateBoost')
    for method_name in names:
        traits=[t for t in cls['traits'] if t['kind']==1 and t['name'].endswith('.'+method_name)]
        if len(traits)!=1:raise ValueError('El HUD no tiene el método de presentación esperado.')
        mid=traits[0]['method'];record=abc.bodies[mid]
        if record['exceptions']:raise ValueError('El método del HUD cambió de estructura.')
        patched,edits=neutralize_color_method(record['code'],abc.name,identity,method_name=='onTeamColorChanged')
        at=start+record['codepos'];out[at:at+len(patched)]=patched;changed[mid]=record['code'];sites+=len(edits)
        allowed_bytes.update(at+i for a,b in edits for i in range(a,b))
    checked=abc_tag(bytes(out))[-1]
    for index,old in abc.bodies.items():
        if index not in changed and checked.bodies[index]['code']!=old['code']:raise ValueError('Cambió otro método del HUD.')
    if any(a!=b and i not in allowed_bytes for i,(a,b) in enumerate(zip(data,out))):raise ValueError('Cambio fuera de las referencias de color de presentación.')
    colored=white_meter_labels(bytes(out))
    return colored,{'boostmeter_text_color':'explicit-white','only_meter_display_methods_changed':True,
                       'boost_amount_calculation_instructions_unchanged':True,'methods':list(names),'color_sites':sites,
                       'label_and_number_textColor_explicit':True,'runtime_visual_verified':False}


def white_meter_labels(movie):
    """Finish native meter updates with explicit label colors; identity alone
    preserves colors baked into the original text field. Never edits global HUD.
    """
    from title_display import _ref
    start,body,end,pos,abc=abc_tag(movie);edits=[];changed={}
    def prop(name):return b'\x66'+encode_u30(_ref(abc,name))
    def setprop(name):return b'\x61'+encode_u30(_ref(abc,name))
    try:white=abc.ints.index(0xffffff)
    except ValueError:raise ValueError('No se identifica el blanco nativo del medidor.')
    # Use fresh transforms rather than a shared static object. Color tweens can
    # write after an update returns; cancel only their colorTransform property,
    # leaving progress/amount and alpha animations intact.
    transform=_ref(abc,'flash.geom.ColorTransform')
    normal=b'\x5d'+encode_u30(transform)+b'\x4a'+encode_u30(transform)+b'\x00'
    tween=b'\x60'+encode_u30(_ref(abc,'com.greensock.TweenMax'))
    color_string=b'\x2c'+encode_u30(abc.strings.index('colorTransform'))
    suffix=b'\xd0'+prop('transform')+normal+setprop('colorTransform')
    fields=[b'\xd0'+prop(name) for name in ['backgroundMovieClip','fillProgressBar','fillProgressBarTinted','glowMovieClip','backgroundTextField','boostTextField','boostLabel']]
    fields.append(b'\xd0'+prop('boostLabel')+prop('textField'))
    for field in fields:
        suffix+=tween+field+b'\x27'+color_string+b'\x26\x55\x01\x4f'+encode_u30(_ref(abc,'killTweensOf'))+b'\x03'
        suffix+=field+prop('transform')+normal+setprop('colorTransform')
    for field in [b'\xd0'+prop('boostTextField'),b'\xd0'+prop('boostLabel')+prop('textField')]:
        suffix+=field+b'\x2d'+encode_u30(white)+setprop('textColor')
        suffix+=field+prop('transform')+normal+setprop('colorTransform')
    for t in abc.classes['tagame.hud.BoostMeterView']['traits']:
        if t['kind']!=1 or t['name'].split('.')[-1] not in ['onTeamColorChanged','onBoostChanged','updateBoost']:continue
        mid=t['method'];record=abc.bodies[mid]
        if record['exceptions'] or record['code'][-1:]!=b'\x47' or record['stack']<5:raise ValueError('Método de medidor no reconocido.')
        code=record['code'][:-1]+suffix+b'\x47';changed[mid]=code
        edits.append((record['lengthpos'],record['codepos']+len(record['code']),encode_u30(len(code))+code))
    if len(changed)!=3:raise ValueError('No se verificaron las tres actualizaciones del medidor.')
    data=abc.data
    for a,b,value in sorted(edits,reverse=True):data=data[:a]+value+data[b:]
    after=ABC(data)
    for mid,record in abc.bodies.items():
        if after.bodies[mid]['code']!=changed.get(mid,record['code']):raise ValueError('Cambió otro método del HUD.')
    payload=movie[body:pos]+data
    result=bytearray(movie[:start]+struct.pack('<HI',(82<<6)|63,len(payload))+payload+movie[end:])
    struct.pack_into('<I',result,4,len(result));abc_tag(result)
    return bytes(result)

def patch_boostmeter_movie(pkg,flat):
    from cosmetic_colors import export_properties
    import zlib
    from modern_upk import i64,i32
    movies=[(i,e) for i,e in enumerate(pkg.exports) if pkg.class_name(e)=='SwfMovie' and pkg.path(i)=='GFX_Hud.HUD']
    if len(movies)!=1:raise ValueError('No se identifica la película del boostmeter.')
    index,exp=movies[0];data,props=export_properties(pkg,flat,exp)
    raw=next((p for p in props if p['name']=='RawData' and p['kind']=='ArrayProperty'),None)
    if not raw or i32(data,raw['value'])!=raw['size']-4:raise ValueError('RawData del HUD no válido.')
    at=raw['value']+4;old=data[at:at+raw['size']-4];new,report=neutral_boostmeter_gfx(old)
    result=bytearray(data[:at]+new+data[at+len(old):])
    struct.pack_into('<i',result,raw['value']-8,4+len(new));struct.pack_into('<i',result,raw['value'],len(new))
    crc=next((p for p in props if p['name']=='SourceFileCRC' and p['size']==4),None)
    if crc:struct.pack_into('<I',result,crc['value']+(len(new)-len(old) if crc['value']>at else 0),zlib.crc32(new)&0xffffffff)
    return index,bytes(result),report
