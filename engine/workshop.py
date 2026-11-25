"""Public map catalog and local library. Only Utopia Retro may be replaced."""
from pathlib import Path, PurePosixPath
import concurrent.futures, hashlib, json, os, shutil, struct, tempfile, uuid, zipfile
from urllib.request import Request, urlopen
from urllib.error import HTTPError, URLError
from urllib.parse import urlparse, urlencode
from html.parser import HTMLParser
from swap_manager import sha, pristine, load_state
from menu_guard import require_closed

TARGET='Labs_Utopia_P.upk'
CATALOG='https://lethamyr.com/api/v1/maps'
LIMIT=768*1024*1024

def folder(api):
    p=api.ROOT/'data/workshop';p.mkdir(parents=True,exist_ok=True);return p

def write(path,data):
    temp=path.with_suffix('.tmp');temp.write_text(json.dumps(data,ensure_ascii=False,indent=2),encoding='utf8');os.replace(temp,path)

def read(path,default):
    return json.loads(path.read_text(encoding='utf8')) if path.exists() else default

def https(url):
    parsed=urlparse(url)
    if parsed.scheme!='https' or parsed.username or parsed.password:raise ValueError('Solo enlaces HTTPS públicos.')
    host=(parsed.hostname or '').lower()
    if host not in {'lethamyr.com','www.lethamyr.com','drive.google.com','drive.usercontent.google.com','docs.google.com'} and not host.endswith('.googleusercontent.com'):
        raise ValueError('Fuente de descarga no admitida. Importa el mapa descargado desde su autor.')
    return url

def fetch(url,maximum,label=None):
    https(url)
    try:
        with urlopen(Request(url,headers={'User-Agent':'SwapPrism/0.23 (map catalog)','Accept':'*/*'}),timeout=30) as r:
            https(r.url)
            if label is None:raw=r.read(maximum+1)
            else:
                from job_progress import report
                total=int(r.headers.get('Content-Length') or 0);raw=bytearray()
                while len(raw)<=maximum:
                    chunk=r.read(1<<20)
                    if not chunk:break
                    raw+=chunk;report('download',len(raw)>>20,total>>20,label)
                raw=bytes(raw)
            if len(raw)>maximum:raise ValueError('Archivo demasiado grande.')
            return raw
    except HTTPError as e:
        raise ValueError('Este mapa ya no está disponible en su enlace (error %d). Prueba otro o impórtalo desde la web del autor.'%e.code) from None
    except (URLError,TimeoutError) as e:
        raise ValueError('No se pudo descargar: sin conexión o el servidor no responde.') from None

def snapshot(api):
    base=folder(api);data=read(base/'catalog.json',{'maps':[]});library=read(base/'library.json',{})
    for item in data['maps']:
        key=item['key'];entry=library.get(key)
        if entry:item['local']=entry
        thumb=base/'images'/(key+'.png')
        if thumb.is_file():item['preview']=str(thumb)
    known={m['key'] for m in data['maps']}
    data['maps'] += [dict(v,key=k,local=v,source='Importado') for k,v in library.items() if k not in known]
    return data

def refresh(api):
    maps=[];url=CATALOG;seen=set()
    for page in range(30):
        if url in seen:raise ValueError('Paginación repetida del catálogo.')
        seen.add(url);data=json.loads(fetch(url,4*1024*1024))
        for item in data['data']:
            name=item['name'];download=item.get('download_url','')
            key=hashlib.sha256((name+'\n'+download).encode()).hexdigest()
            maps.append({'key':key,'name':name,'description':item.get('description_short',''),'url':download,'image':next(iter(item.get('media',[])),''),'downloads':item.get('counts',{}).get('downloads',0),'source':'Lethamyr.com'})
        url=data.get('links',{}).get('next')
        if not url:break
        if urlparse(url).netloc!='lethamyr.com' or urlparse(url).path!='/api/v1/maps':raise ValueError('Paginación no válida.')
    else:raise ValueError('Catálogo demasiado grande.')
    maps=list({m['key']:m for m in maps}.values())
    base=folder(api);images=base/'images';images.mkdir(exist_ok=True)
    def thumbnail(item):
        dest=images/(item['key']+'.png')
        if dest.exists() or not item['image']:return
        try:
            import io
            from PIL import Image
            raw=fetch(item['image'],8*1024*1024)
            with Image.open(io.BytesIO(raw)) as im:
                if im.width*im.height>30_000_000:return
                im.thumbnail((480,270));im.convert('RGB').save(dest,format='PNG')
        except Exception:pass
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:list(pool.map(thumbnail,maps))
    write(base/'catalog.json',{'maps':maps,'source':CATALOG});return snapshot(api)

def validate_map(path):
    path=Path(path)
    if not 64<=path.stat().st_size<=LIMIT:raise ValueError('Tamaño del mapa no válido.')
    with path.open('rb') as stream:header=stream.read(64)
    if header[:4]!=b'\xc1\x83\x2a\x9e':raise ValueError('No es un paquete Unreal de Rocket League.')
    # Real UDK map packages contain their own World object; header validation is
    # deliberately not presented as a runtime/dependency compatibility guarantee.
    return sha(path)

def import_map(api,path,metadata=None):
    source=Path(path);base=folder(api);meta=metadata or {};library=read(base/'library.json',{})
    if source.suffix.lower() not in {'.zip','.udk','.upk'}:raise ValueError('Importa ZIP, UDK o UPK.')
    if source.stat().st_size>LIMIT:raise ValueError('Archivo demasiado grande.')
    with tempfile.TemporaryDirectory(dir=base,prefix='import-') as tmp:
        tmp=Path(tmp)
        if source.suffix.lower()=='.zip':
            with zipfile.ZipFile(source) as z:
                members=z.infolist()
                if len(members)>2048 or sum(v.file_size for v in members)>LIMIT:raise ValueError('ZIP demasiado grande.')
                for member in members:
                    p=PurePosixPath(member.filename.replace('\\','/'))
                    if p.is_absolute() or '..' in p.parts or ':' in str(p) or (member.external_attr>>16)&0o170000==0o120000:raise ValueError('Ruta no válida dentro del ZIP.')
                candidates=[v for v in members if not v.is_dir() and Path(v.filename).suffix.lower()=='.udk']
                if not candidates:candidates=[v for v in members if not v.is_dir() and Path(v.filename).suffix.lower()=='.upk']
                if len(candidates)!=1:raise ValueError('El ZIP debe contener un único mapa. Extrae e importa el UDK principal.')
                selected=candidates[0];prepared=tmp/'map.upk'
                with z.open(selected) as inp,prepared.open('wb') as out:shutil.copyfileobj(inp,out)
        else:prepared=source
        digest=validate_map(prepared);key=meta.get('key') or digest
        if not isinstance(key,str) or len(key)!=64 or any(c not in '0123456789abcdef' for c in key):raise ValueError('Identificador de biblioteca no válido.')
        dest=base/'maps'/key;dest.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(prepared,dest/'map.upk')
        entry={'name':meta.get('name') or source.stem,'path':str(dest/'map.upk'),'sha256':digest,'source':meta.get('source','Importado'),'key':key}
        library[key]=entry;write(base/'library.json',library)
    return snapshot(api)

class DriveForm(HTMLParser):
    def __init__(self):super().__init__();self.active=False;self.action='';self.fields={}
    def handle_starttag(self,tag,attrs):
        a=dict(attrs)
        if tag=='form' and a.get('id')=='download-form':self.active=True;self.action=a.get('action','')
        if tag=='input' and self.active and a.get('type')=='hidden' and a.get('name') in {'id','export','confirm','uuid'}:self.fields[a['name']]=a.get('value','')
    def handle_endtag(self,tag):
        if tag=='form':self.active=False

def download(api,key,confirmed=False):
    item=next((m for m in snapshot(api)['maps'] if m['key']==key),None)
    if not item or not item.get('url'):raise ValueError('El mapa no tiene enlace de descarga. Usa Importar.')
    base=folder(api)
    # Public files only; never uses accounts, cookies, or executable downloaders.
    raw=fetch(item['url'],LIMIT,'Descargando '+item['name'].strip()+' (MB)')
    if not raw.startswith(b'PK') and urlparse(item['url']).hostname=='drive.google.com':
        form=DriveForm();form.feed(raw[:100000].decode('utf8',errors='replace'))
        if form.action=='https://drive.usercontent.google.com/download' and form.fields.get('id') and form.fields.get('confirm'):
            raw=fetch(form.action+'?'+urlencode(form.fields),LIMIT,'Descargando '+item['name'].strip()+' (MB)')
    if not raw.startswith(b'PK'):raise ValueError('La fuente no entregó un ZIP directo. Descárgalo desde la web del autor y usa Importar.')
    with tempfile.TemporaryDirectory(dir=base,prefix='download-') as tmp:
        path=Path(tmp)/'map.zip';path.write_bytes(raw);return import_map(api,path,item)

def remove(api,key):
    base=folder(api);library=read(base/'library.json',{});entry=library.get(key)
    if not entry:raise ValueError('El mapa no está descargado.')
    state=load_state(api.ROOT/'data').get(TARGET,{})
    if state.get('status')=='installed' and state.get('installed_sha256')==entry['sha256']:raise ValueError('Restaura Utopia Retro antes de borrar el mapa activo.')
    path=Path(entry['path']).resolve();expected=(base/'maps'/key/'map.upk').resolve()
    if path!=expected or path.parent.parent!=(base/'maps').resolve():raise ValueError('Ruta de biblioteca no válida.')
    path.unlink(missing_ok=True);path.parent.rmdir();library.pop(key);write(base/'library.json',library);return snapshot(api)

def prepare(api,game,selection):
    require_closed()
    if selection.get('target',TARGET)!=TARGET:raise ValueError('El destino es exclusivamente Utopia Retro.')
    source=pristine(Path(game)/'TAGame/CookedPCConsole',TARGET,api.ROOT/'data')
    if selection['kind']=='workshop_restore':raw=source;label='Original';restore=True
    else:
        entry=read(folder(api)/'library.json',{}).get(selection.get('map_key'))
        if not entry:raise ValueError('Descarga o importa el mapa primero.')
        raw=Path(entry['path']).resolve()
        if raw.parent.parent!=(folder(api)/'maps').resolve() or validate_map(raw)!=entry['sha256']:raise ValueError('El mapa de biblioteca ha cambiado.')
        label=entry['name'];restore=False
    job=api.ROOT/'paquetes'/('workshop-'+uuid.uuid4().hex);(job/'generado').mkdir(parents=True)
    shutil.copyfile(raw,job/'generado'/TARGET)
    item={'Product':'Workshop · Utopia Retro','Slot':'Workshop','AssetPackage':TARGET}
    report={'kind':'workshop','target':item,'donor':{'Product':label,'Slot':'Workshop'},'original_sha256':{TARGET:sha(source)},'generated_sha256':sha(raw),'validation':{'workshop_map_validated':True,'fixed_utopia_retro_only':True},'restore_selection':restore}
    write(job/'informe.json',report);return job
