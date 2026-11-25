"""Local-only avatar frame. Leaves all original avatar textures/frames intact.

Bitmap/shape encoding follows Adobe SWF 10, DefineBitsLossless2/DefineShape3.
These tags are supported by Scaleform GFx; no external image loader is needed.
"""
import io,struct,zlib
from PIL import Image,ImageOps
from hud_abc import ABC,Reader,encode_u30,swf_tags
from title_display import _ref
from fake_display import _getter

FRAME='swap_prism_local'
def tag(code,data=b''):
    return struct.pack('<HI',(code<<6)|63,len(data))+data

class Bits:
    def __init__(self):self.bits=[]
    def put(self,value,count):self.bits.extend((value>>i)&1 for i in range(count-1,-1,-1))
    def finish(self):
        self.bits.extend([0]*((-len(self.bits))%8))
        return bytes(sum(self.bits[i+j]<<(7-j) for j in range(8)) for i in range(0,len(self.bits),8))

SIDE=128 # source pixels; the shape stays 64 px (1280 twips)

def bitmap_shape(bitmap,shape,raw):
    with Image.open(io.BytesIO(raw)) as source:image=ImageOps.fit(source.convert('RGBA'),(SIDE,SIDE),Image.Resampling.LANCZOS)
    pixels=bytearray()
    for r,g,b,a in image.getdata():pixels.extend((a,(r*a+127)//255,(g*a+127)//255,(b*a+127)//255))
    bitmap_tag=tag(36,struct.pack('<HBHH',bitmap,5,SIDE,SIDE)+zlib.compress(bytes(pixels)))
    rect=Bits();rect.put(12,5)
    for value in [0,1280,0,1280]:rect.put(value,12)
    matrix=Bits();matrix.put(1,1);matrix.put(22,5);matrix.put((1280<<16)//SIDE,22);matrix.put((1280<<16)//SIDE,22);matrix.put(0,1);matrix.put(0,5)
    edges=Bits();edges.put(1,4);edges.put(0,4) # one fill index bit, no lines
    edges.put(0,1);edges.put(5,5);edges.put(1,5);edges.put(0,1);edges.put(0,1);edges.put(1,1)
    for vertical,delta in [(False,1280),(True,1280),(False,-1280),(True,-1280)]:
        edges.put(1,1);edges.put(1,1);edges.put(10,4);edges.put(0,1);edges.put(int(vertical),1);edges.put(delta,12)
    edges.put(0,6)
    shape_tag=tag(32,struct.pack('<H',shape)+rect.finish()+b'\x01\x41'+struct.pack('<H',bitmap)+matrix.finish()+b'\x00'+edges.finish())
    return bitmap_tag+shape_tag

def add_local_frame(movie,raw):
    records=list(swf_tags(movie));symbol=None
    for code,start,body,end in records:
        if code!=76:continue
        data=movie[body:end];at=2
        for _ in range(struct.unpack_from('<H',data)[0]):
            ident=struct.unpack_from('<H',data,at)[0];at+=2;stop=data.index(0,at)
            if data[at:stop]==b'TintedAvatarIcons':symbol=ident
            at=stop+1
    if symbol is None:return movie,False
    candidates=[(start,body,end) for code,start,body,end in records if code==39 and struct.unpack_from('<H',movie,body)[0]==symbol]
    if len(candidates)!=1:raise ValueError('No se identifica el sprite de avatar compartido.')
    start,body,end=candidates[0];data=movie[body:end]
    if FRAME.encode()+b'\0' in data:raise ValueError('El sprite ya contiene una capa de avatar.')
    used={struct.unpack_from('<H',movie,b)[0] for _,_,b,e in records if e-b>=2}
    bitmap,shape=65000,65001
    if bitmap in used or shape in used:raise ValueError('Identificadores de avatar ocupados.')
    # Preserve all original timeline records and all native frames byte for byte.
    # The existing default placement's matrix determines the exact avatar size.
    at=4;matrix=None;depth=None;end_at=None
    while at<len(data):
        record_at=at;head=struct.unpack_from('<H',data,at)[0];at+=2;n=head&63;code=head>>6
        if n==63:n=struct.unpack_from('<I',data,at)[0];at+=4
        value=data[at:at+n];at+=n
        if code==70 and matrix is None:
            if value[:2]!=b'\x06\x10':raise ValueError('La colocación nativa del avatar cambió.')
            depth=struct.unpack_from('<H',value,2)[0];matrix=value[6:]
        if code==0:end_at=record_at;break
    if matrix is None or end_at is None:raise ValueError('Timeline de avatar incompleto.')
    extra=tag(28,struct.pack('<H',depth))+tag(43,FRAME.encode()+b'\0')+tag(26,b'\x06'+struct.pack('<HH',depth,shape)+matrix)+tag(1)+tag(0)
    new=struct.pack('<HH',symbol,struct.unpack_from('<H',data,2)[0]+1)+data[4:end_at]+extra
    result=bytearray(movie[:start]+bitmap_shape(bitmap,shape,raw)+tag(39,new)+movie[end:])
    struct.pack_into('<I',result,4,len(result));list(swf_tags(result))
    return bytes(result),True

def patch_avatar_abc(data):
    abc=ABC(data);cls=abc.classes.get('tagame.controls.PlayerAvatar',{})
    methods=[t['method'] for t in cls.get('traits',[]) if t['kind']==1 and t['name'].endswith('.validateAvatarIcon')]
    if not methods:return data,[]
    mid=methods[0];body=abc.bodies[mid]
    local_id=_getter(abc,'tagame.api.LocalPlayer',True,'PlayerID')
    reader=Reader(body['code'])
    while reader.data[reader.pos] in (0xf0,0xf1):reader.byte();reader.u30()
    if len(methods)!=1 or local_id is None or abc.name(abc.methods[local_id]['return'])!='String' or body['exceptions'] or reader.byte()!=0xd0 or reader.byte()!=0x30:
        raise ValueError('No se puede verificar la identidad local del avatar.')
    scope=reader.pos
    def prop(name):return b'\x66'+encode_u30(_ref(abc,name))
    def setprop(name):return b'\x61'+encode_u30(_ref(abc,name))
    tinted=b'\xd0'+prop('tintedAvatarIcons');loader=b'\xd0'+prop('avatarIconLoader')
    # The team colour is applied to the whole TintedAvatarIcons sprite; the photo keeps its own colours.
    untint=tinted+prop('transform')+loader+prop('transform')+prop('colorTransform')+setprop('colorTransform')
    render=loader+b'\x27'+setprop('visible')+tinted+b'\x26'+setprop('visible')
    render+=tinted+b'\x2c'+encode_u30(len(abc.strings))+b'\x4f'+encode_u30(_ref(abc,'gotoAndStop'))+b'\x01'+untint+b'\x47'
    def guard(check,code):return check+b'\x12'+len(code).to_bytes(3,'little')+code
    row=b'\xd0'+prop('tagame.controls:PlayerAvatar.m_playerInfoRow')
    ident=b'\xd0'+prop('tagame.controls:PlayerAvatar.m_playerId')
    def local_only(code):
        fallback=guard(row,guard(row+prop('bLocalPlayer'),code))
        skip=b'\x10'+len(fallback).to_bytes(3,'little')
        compare=ident+b'\x60'+encode_u30(_ref(abc,'tagame.api.LocalPlayer'))+prop('PlayerID')+b'\x14'+(len(code)+len(skip)+len(fallback)).to_bytes(3,'little')+code+skip
        return guard(ident,compare)+fallback
    code=body['code'][:scope]+local_only(render)+body['code'][scope:]
    edits=[(body['lengthpos'],body['codepos']+len(body['code']),encode_u30(len(code))+code),
           (abc.string_pool_end,abc.string_pool_end,encode_u30(len(FRAME))+FRAME.encode())]
    changed={mid:code}
    tint=[t['method'] for t in cls.get('traits',[]) if t['kind']==1 and t['name'].endswith('.updateTeamColor')]
    if len(tint)==1 and abc.bodies[tint[0]]['code'][-1:]==b'\x47' and not abc.bodies[tint[0]]['exceptions']:
        # Runs after every team-colour update; inserted just before the final returnvoid.
        tbody=abc.bodies[tint[0]];tcode=tbody['code'][:-1]+local_only(untint+b'\x47')+b'\x47';changed[tint[0]]=tcode
        edits.append((tbody['lengthpos'],tbody['codepos']+len(tbody['code']),encode_u30(len(tcode))+tcode))
    r=Reader(data);r.pos=abc.string_count_pos;r.u30();edits.append((abc.string_count_pos,r.pos,encode_u30(len(abc.strings)+1)))
    result=data
    for start,end,repl in sorted(edits,reverse=True):result=result[:start]+repl+result[end:]
    after=ABC(result)
    if after.classes!=abc.classes or after.methods!=abc.methods:raise ValueError('Cambió una definición del avatar.')
    for method,old in abc.bodies.items():
        if after.bodies[method]['code']!=changed.get(method,old['code']):raise ValueError('Cambió otro método del avatar.')
    return result,['tagame.controls.PlayerAvatar.localOnlyImage']+(['tagame.controls.PlayerAvatar.localUntinted'] if len(changed)>1 else [])
