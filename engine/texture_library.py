"""Import AlphaConsole image/config data. Never extract or execute archive code."""
from pathlib import Path,PurePosixPath
from dataclasses import dataclass,field
import hashlib,io,json,zipfile
from PIL import Image

@dataclass
class TexturePack:
    name:str
    category:str
    source:str
    layers:dict
    body_id:int|None=None
    skin_id:int|None=None
    digest:str=''

def safe_member(name):
    p=PurePosixPath(name.replace('\\','/'))
    if p.is_absolute() or '..' in p.parts or ':' in name:raise ValueError('Ruta fuera del pack.')
    return str(p)

def scan(root):
    root=Path(root); packs=[]; errors=[]
    sources=[root] if root.is_file() else sorted(root.rglob('*.json'))+sorted(root.rglob('*.zip'))
    seen=set()
    for source in sources:
        try:
            for pack in read(source):
                if pack.digest not in seen: packs.append(pack);seen.add(pack.digest)
        except (ValueError,OSError,KeyError,zipfile.BadZipFile) as exc:errors.append(f'{source.name}: {exc}')
    return packs,errors

def read(source):
    source=Path(source)
    if source.suffix.casefold() in ('.png','.dds'):
        raw=source.read_bytes()
        with Image.open(io.BytesIO(raw)) as im:
            if im.width*im.height>4096*4096:raise ValueError('Imagen supera 4096².')
            im.verify()
        return [TexturePack(source.stem,'Boostmeter',str(source),{'Background':raw},digest=hashlib.sha256(raw).hexdigest())]
    if source.suffix.casefold()=='.zip':
        if source.stat().st_size>256*1024*1024:raise ValueError('ZIP demasiado grande.')
        with zipfile.ZipFile(source) as z:
            infos=z.infolist()
            if len(infos)>2000 or sum(i.file_size for i in infos)>512*1024*1024:raise ValueError('ZIP supera los límites.')
            names={safe_member(i.filename):i for i in infos if not i.is_dir()}
            data={n:z.read(i) for n,i in names.items() if Path(n).suffix.casefold() in ('.json','.png','.jpg','.jpeg','.dds')}
        configs=[n for n in data if n.casefold().endswith('.json')]
    elif source.suffix.casefold()=='.json':
        data={source.name:source.read_bytes()};configs=[source.name]
    else:raise ValueError('Selecciona ZIP, JSON, PNG o DDS.')
    result=[]
    for config in configs:
        if len(data[config])>1024*1024:raise ValueError('JSON demasiado grande.')
        obj=json.loads(data[config].decode('utf-8-sig'))
        if not isinstance(obj,dict):continue
        for name,record in obj.items():
            if not isinstance(record,dict):continue
            category='Boostmeter' if 'Background' in record and 'Fill' in record else 'Pelota' if 'Params' in record else 'Decal' if 'Body' in record else ''
            if not category:continue
            params=record if category=='Boostmeter' else record.get('Params',record.get('Body',{}))
            layers={}
            for role,filename in params.items():
                if not isinstance(filename,str) or not filename.strip():continue
                # AlphaConsole packs sometimes use an extension-only placeholder
                # for an intentionally absent layer, e.g. Sakura's Background.
                if filename.strip().casefold() in {'.png','.dds','.jpg','.jpeg'}:continue
                relative=safe_member(filename)
                key=safe_member(str(PurePosixPath(config).parent/relative))
                if source.suffix.casefold()=='.zip':
                    matched=next((n for n in data if n.casefold()==key.casefold()),None)
                    if matched is None:raise ValueError('Falta imagen: '+relative)
                    raw=data[matched]
                else:
                    file=(source.parent/relative).resolve()
                    if source.parent.resolve() not in file.parents:raise ValueError('Imagen fuera del pack.')
                    if file.stat().st_size>64*1024*1024:raise ValueError('Imagen demasiado grande.')
                    raw=file.read_bytes()
                if len(raw)>64*1024*1024:raise ValueError('Imagen demasiado grande.')
                with Image.open(io.BytesIO(raw)) as im:
                    if im.width*im.height>4096*4096:raise ValueError('Imagen supera 4096².')
                    im.verify()
                layers[role]=raw
            if not layers:continue
            # Equal directory/ZIP copies produce one library entry.
            digest=hashlib.sha256(category.encode()+str(record.get('SkinID','')).encode()+b''.join(k.encode()+hashlib.sha256(v).digest() for k,v in sorted(layers.items()))).hexdigest()
            result.append(TexturePack(str(name),category,str(source),layers,record.get('BodyID'),record.get('SkinID'),digest))
    return result

def image_for(pack):
    role=next((r for r in ('1_Diffuse_Skin','Diffuse','Background','Fill') if r in pack.layers),next(iter(pack.layers)))
    return Image.open(io.BytesIO(pack.layers[role])).convert('RGBA')

def save_library(folder,packs):
    folder=Path(folder);folder.mkdir(parents=True,exist_ok=True)
    for pack in packs:
        layers={}
        for role,raw in pack.layers.items():
            name=hashlib.sha256(raw).hexdigest()+'.image'
            file=folder/name
            if not file.exists():file.write_bytes(raw)
            layers[role]=name
        record={k:getattr(pack,k) for k in ('name','category','source','body_id','skin_id','digest')};record['layers']=layers
        (folder/(pack.digest+'.json')).write_text(json.dumps(record,ensure_ascii=False),encoding='utf-8')

def load_library(folder):
    folder=Path(folder);result=[]
    if not folder.is_dir():return result
    for config in folder.glob('*.json'):
        try:
            obj=json.loads(config.read_text(encoding='utf-8'));layers={}
            for role,name in obj.pop('layers').items():
                file=(folder/safe_member(name)).resolve()
                if file.parent!=folder.resolve():raise ValueError('Ruta inválida.')
                if file.stat().st_size>64*1024*1024:raise ValueError('Imagen demasiado grande.')
                layers[role]=file.read_bytes()
            if obj.get('category')=='Boost':
                obj['category']='Boostmeter';layers={'Background':layers['Diffuse']} if 'Diffuse' in layers else layers
            result.append(TexturePack(layers=layers,**obj))
        except (ValueError,OSError,TypeError,KeyError):continue
    return result
