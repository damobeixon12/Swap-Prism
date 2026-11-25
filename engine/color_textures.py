"""Decode and neutralize material-bound diffuse colour. Never touches normals/masks."""
from pathlib import Path
import io,struct,zlib
from PIL import Image,ImageChops
from modern_upk import i32,i64,Package

def diffuse_textures(pkg):
    from texture_engine import bindings
    found=set()
    for name,indexes in bindings(pkg,pkg.flat()).items():
        low=name.casefold()
        if any(s in low for s in ('diffuse','albedo','basecolor','rimdiffuse')) and not any(s in low for s in ('normal','mask','emiss','noise','uv','offset')):
            found.update(indexes)
    return found

def decompress_bulk(data):
    if len(data)<16 or struct.unpack_from('<I',data,0)[0]!=0x9e2a83c1:return data
    total=i32(data,12);cursor=16;blocks=[];size=0
    while size<total:
        comp,raw=struct.unpack_from('<ii',data,cursor);cursor+=8
        if comp<=0 or raw<=0 or total>64*1024*1024:raise ValueError('Bulk fuera de rango.')
        blocks.append((comp,raw));size+=raw
    out=bytearray()
    for comp,raw in blocks:
        block=zlib.decompress(data[cursor:cursor+comp]);cursor+=comp
        if len(block)!=raw:raise ValueError('Bulk inválido.')
        out.extend(block)
    return bytes(out)

def decode_texture(pkg,index,cooked):
    from texture_engine import parse_props,mip_table
    exp=pkg.exports[index];flat=pkg.flat();data=flat[i64(exp,36):i64(exp,36)+i32(exp,32)]
    _,end,props=parse_props(pkg,data);_,_,levels=mip_table(data,end)
    fmt=next((pkg.names[i32(data,p['value'])] for p in props if p['name']=='Format'),'')
    cache=next((pkg.names[i32(data,p['value'])] for p in props if p['name']=='TextureFileCacheName'),'None')
    candidates=[]
    for pos,flags,disk,w,h in levels:
        if disk<=0 or flags&0x20 or w*h>4096*4096:continue
        if flags&1:
            if not cooked or cache=='None' or any(c in cache for c in ('/','\\',':')):continue
            file=Path(cooked)/(cache+'.tfc');offset=i64(data,pos+12)
            if not file.is_file() or offset<0 or offset+disk>file.stat().st_size:continue
            with file.open('rb') as stream:stream.seek(offset);raw=stream.read(disk)
        else:raw=data[pos+12:pos+12+disk]
        candidates.append((w*h,w,h,raw))
    for _,w,h,raw in sorted(candidates,reverse=True):
        try:
            raw=decompress_bulk(raw)
            if fmt=='PF_A8R8G8B8':return Image.frombytes('RGBA',(w,h),raw,'raw','BGRA')
            if fmt not in ('PF_DXT1','PF_DXT5'):continue
            header=b'DDS '+struct.pack('<7I',124,0xA1007,h,w,len(raw),0,0)+bytes(44)+struct.pack('<II4s5I',32,4,fmt[3:].encode(),0,0,0,0,0)+struct.pack('<5I',0x1000,0,0,0,0)
            with Image.open(io.BytesIO(header+raw)) as im:return im.convert('RGBA')
        except (ValueError,OSError,zlib.error):continue
    raise ValueError('No se pudo leer la textura de color '+pkg.path(index))

def neutralize_diffuse(pkg,cooked,color,has_rgb_parameters):
    from texture_engine import inline_serial,append_exports
    from cosmetic_colors import PAINTS
    candidates=diffuse_textures(pkg);replacements={};names=[]
    flat=pkg.flat()
    for index in candidates:
        image=decode_texture(pkg,index,cooked);r,g,b,a=image.split();value=ImageChops.lighter(ImageChops.lighter(r,g),b)
        if has_rgb_parameters:out=Image.merge('RGBA',(value,value,value,a))
        else:
            # PNG stores sRGB; palette values are linear material RGB.
            def srgb(x):return 12.92*x if x<=.0031308 else 1.055*x**(1/2.4)-.055
            rgb=[min(1,max(0,srgb(x))) for x in PAINTS[color]]
            channels=[value.point([round(i*c) for i in range(256)]) for c in rgb];out=Image.merge('RGBA',(*channels,a))
        raw=io.BytesIO();out.save(raw,format='PNG');exp=pkg.exports[index]
        replacements[index]=inline_serial(pkg,flat[i64(exp,36):i64(exp,36)+i32(exp,32)],raw.getvalue());names.append(pkg.path(index))
    if not replacements:return pkg,[]
    return Package(pkg.file,pkg.engine,raw=append_exports(pkg,replacements)),names
