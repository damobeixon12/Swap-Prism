"""Public creator catalogs and local previews. Downloads never apply game changes."""
from pathlib import Path
from html.parser import HTMLParser
from urllib.request import Request,urlopen
from urllib.parse import urlparse,urljoin,urlencode
import json,re,hashlib,tempfile,concurrent.futures,io
from PIL import Image
import studio_backend as studio
from texture_library import scan,save_library

SOURCES={'alpha':'https://alphaconsole.io/browse','vgm':'https://videogamemods.com/vgm/rocketleague/mods?page=3','sounds':'https://www.myinstants.com/es/search/'}
VGM=['sakura-boost-meter-1020466','car-meter-1022094','pro-team-boost-gauge-1021662','tribal-fire-boost-meter','ice-dark-boost-meter']
HOSTS={'alphaconsole.io','s3.alphaconsole.io','videogamemods.com','www.videogamemods.com','uploads.videogamemods.com','myinstants.com','www.myinstants.com'}

class Node:
    def __init__(self,tag='',attrs=()):self.tag=tag;self.a=dict(attrs);self.children=[];self.text=''
    def all(self,tag=None):
        for node in self.children:
            if tag is None or node.tag==tag:yield node
            yield from node.all(tag)
    def content(self):return ' '.join((self.text+' '+' '.join(n.content() for n in self.children)).split())

class DOM(HTMLParser):
    def __init__(self,text):
        super().__init__(convert_charrefs=True);self.root=Node();self.stack=[self.root];self.feed(text)
    def handle_starttag(self,tag,attrs):
        n=Node(tag,attrs);self.stack[-1].children.append(n)
        if tag not in {'img','br','hr','input','meta','link','source','area','wbr','embed'}:self.stack.append(n)
    def handle_endtag(self,tag):
        for i in range(len(self.stack)-1,0,-1):
            if self.stack[i].tag==tag:self.stack=self.stack[:i];break
    def handle_data(self,text):self.stack[-1].text+=text

def allowed(url):
    p=urlparse(url)
    if p.scheme!='https' or p.hostname not in HOSTS or p.username or p.password:raise ValueError('Usa un enlace HTTPS de AlphaConsole, VGM o Myinstants.')
    return url

def fetch(url,limit=5*1024*1024):
    allowed(url)
    with urlopen(Request(url,headers={'User-Agent':'SwapPrism/0.25 (public cosmetic browser)'}),timeout=30) as response:
        allowed(response.url);raw=response.read(limit+1)
        if len(raw)>limit:raise ValueError('La descarga supera el tamaño permitido.')
        return raw

def base(api):
    p=api.ROOT/'data/media';p.mkdir(parents=True,exist_ok=True);return p

def load(api,source):
    p=base(api)/(source+'.json');return json.loads(p.read_text(encoding='utf8')) if p.exists() else {'items':[],'source':source}

def write(api,source,data):
    p=base(api)/(source+'.json');tmp=p.with_suffix('.tmp');tmp.write_text(json.dumps(data,ensure_ascii=False),encoding='utf8');tmp.replace(p)

def make(source,name,page,category,download='',image='',author=''):
    return {'key':hashlib.sha256(page.encode()).hexdigest(),'source':source,'name':name,'page':page,'category':category,'download':download,'image':image,'author':author}

def alpha(html):
    result=[]
    for card in DOM(html).root.all('div'):
        if not {'card','ratio','hover-zoom'}.issubset(set(card.a.get('class','').split())):continue
        image=next((n for n in card.all('img') if 'preview-image-img' in n.a.get('class','')),None)
        link=next((n for n in card.all('a') if n.a.get('href','').startswith('/view/')),None)
        label=next((n for n in card.all('div') if 'item-label-' in n.a.get('class','')),None)
        if not image or not link or not label:continue
        category=next((v for k,v in [('item-label-boostmeter','Boostmeter'),('item-label-ball','Pelota'),('item-label-decal','Decal')] if k in label.a.get('class','').split()),None)
        if not category:continue
        author=next((n.content() for n in card.all('a') if n.a.get('href','').startswith('/user/')),'')
        download=next((n.a['action'] for n in card.all('form') if n.a.get('action','').startswith('https://s3.alphaconsole.io/uploads/')),'')
        result.append(make('alpha',image.a.get('title') or image.a.get('alt','Textura'),urljoin(SOURCES['alpha'],link.a['href']),category,download,image.a.get('src',''),author))
    return result

def vgm_detail(url):
    html=fetch(url).decode('utf8');root=DOM(html).root
    meta={n.a.get('property') or n.a.get('name'):n.a.get('content','') for n in root.all('meta')}
    name=meta.get('og:title','').split(' - Rocket League Mod')[0]
    text='\n'.join(n.text for n in root.all('script'))
    # Decode only public Next/RSC text strings, never execute site JavaScript.
    for match in re.finditer(r'self\.__next_f\.push\(\[1,("(?:[^"\\]|\\.)*")\]\)',text):
        try:text+='\n'+json.loads(match[1])
        except ValueError:pass
    path=re.search(r'"file_path"\s*:\s*"([^"\r\n]+\.zip)"',text)
    title=name.lower();category='Boostmeter' if any(v in title for v in ['meter','gauge','hud']) else 'Pelota' if 'ball' in title else 'Decal' if 'decal' in title else 'Otros'
    download=urljoin('https://uploads.videogamemods.com/',path[1]) if path else ''
    return make('vgm',name,url,category,download,meta.get('og:image',''))

def sounds(html):
    result=[]
    for card in DOM(html).root.all('div'):
        if 'instant' not in card.a.get('class','').split():continue
        link=next((n for n in card.all('a') if 'instant-link' in n.a.get('class','').split()),None)
        button=next((n for n in card.all('button') if n.a.get('onclick','').startswith('play(')),None)
        if not link or not button:continue
        audio=re.search(r"play\('(/media/sounds/[^']+\.mp3)'",button.a['onclick'])
        if audio:result.append(make('sounds',link.content(),urljoin(SOURCES['sounds'],link.a['href']),'Sonido',urljoin(SOURCES['sounds'],audio[1])))
    return result

def catalog(api,source,query='',refresh=False,url=''):
    old=load(api,source);items=old['items']
    if refresh:
        if url:
            allowed(url)
            if source=='vgm' and '/vgm/rocketleague/mods/' in url:incoming=[vgm_detail(url)]
            elif source=='alpha' and '/view/' in url:
                html=fetch(url).decode('utf8');root=DOM(html).root
                meta={n.a.get('property'):n.a.get('content','') for n in root.all('meta')}
                download=next((n.a['action'] for n in root.all('form') if n.a.get('action','').startswith('https://s3.alphaconsole.io/uploads/')),'')
                name=meta.get('og:title','Textura AlphaConsole').split('|')[0].strip()
                types={'item-label-boostmeter':'Boostmeter','item-label-ball':'Pelota','item-label-decal':'Decal'}
                category=next((types[c] for node in root.all() for c in node.a.get('class','').split() if c in types),None)
                if not category:raise ValueError('La ficha no es una pelota, boostmeter o decal compatible.')
                incoming=[make('alpha',name,url,category,download,meta.get('og:image',''))]
            else:raise ValueError('Pega la ficha de una textura de la fuente seleccionada.')
        elif source=='alpha':incoming=alpha(fetch(SOURCES[source]).decode('utf8'))
        elif source=='sounds':incoming=sounds(fetch(SOURCES[source]+'?'+urlencode({'name':query})).decode('utf8'))
        elif source=='vgm':
            incoming=[]
            for slug in VGM:
                try:incoming.append(vgm_detail('https://videogamemods.com/vgm/rocketleague/mods/'+slug))
                except Exception:continue
            if not incoming:raise ValueError('VGM no entregó fichas. Puedes abrir su web e importar un ZIP.')
        else:raise ValueError('Fuente no válida.')
        if not incoming:raise ValueError('No se encontraron resultados en esta fuente.')
        previous={m['key']:m for m in items}
        for item in incoming:
            for field in ['local','digests']:
                if field in previous.get(item['key'],{}):item[field]=previous[item['key']][field]
        items=list({m['key']:m for m in items+incoming}.values()) if url else incoming
        images=base(api)/'images';images.mkdir(exist_ok=True)
        def thumbnail(item):
            if not item.get('image'):return
            dest=images/(item['key']+'.png')
            try:
                if not dest.exists():
                    with Image.open(io.BytesIO(fetch(item['image'],8*1024*1024))) as im:
                        if im.width*im.height>25_000_000:return
                        im.thumbnail((420,260));im.convert('RGB').save(dest)
                item['preview']=str(dest)
            except Exception:pass
        with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:list(pool.map(thumbnail,items))
    data={'items':items,'source':source};write(api,source,data);return data

def audio_library(api):
    result=[]
    for path in (api.ROOT/'data/sounds').glob('*.json'):
        try:
            item=json.loads(path.read_text(encoding='utf8'))
            if item.get('kind') in {'post','reset'} and Path(item['path']).is_file():result.append(item)
        except (ValueError,KeyError):continue
    return result

def preview_sound(api,key,kind='post'):
    """Fetch and play a short clip in memory. Never import or mark downloaded."""
    import miniaudio,winsound,wave,math
    data=load(api,'sounds');item=next((m for m in data['items'] if m['key']==key),None)
    if not item or not item.get('download'):raise ValueError('Sonido no disponible para preescucha.')
    spec=studio.manifest(api).get(kind)
    if not spec:raise ValueError('Evento de audio no válido.')
    raw=fetch(item['download'],8*1024*1024);info=miniaudio.mp3_get_info(raw)
    if not math.isfinite(info.duration) or not 0<info.duration<=300:raise ValueError('El audio supera 5 minutos o no es válido.')
    sound=miniaudio.decode(raw,output_format=miniaudio.SampleFormat.SIGNED16,nchannels=1,sample_rate=48000)
    samples=sound.samples[:round(spec['max_seconds']*48000)]
    if not samples:raise ValueError('El sonido está vacío.')
    out=io.BytesIO()
    with wave.open(out,'wb') as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(48000);w.writeframes(samples.tobytes())
    winsound.PlaySound(out.getvalue(),winsound.SND_MEMORY)
    return {'previewed':True,'name':item['name'],'media':data}

def download(api,source,key,kind='post',preview=False):
    if source=='sounds' and preview:return preview_sound(api,key,kind)
    data=load(api,source);item=next((m for m in data['items'] if m['key']==key),None)
    if not item or not item.get('download'):raise ValueError('Esta ficha requiere descarga desde su web. Después usa Importar.')
    local=base(api)/'files';local.mkdir(exist_ok=True)
    if source=='sounds':
        path=local/(key+'.mp3')
        if not path.exists():
            raw=fetch(item['download'],8*1024*1024);path.write_bytes(raw)
        try:audio=studio.import_mp3(api,path,kind)
        except Exception:path.unlink(missing_ok=True);raise
        audio['name']=item['name'][:64];audio['catalog_key']=key;Path(audio['path']).with_suffix('.json').write_text(json.dumps(audio),encoding='utf8')
        item['local']=True;write(api,source,data)
        return {'media':data,'sounds':audio_library(api)}
    raw=fetch(item['download'],128*1024*1024)
    if not raw.startswith(b'PK'):raise ValueError('La fuente no entregó un ZIP. Abre la ficha e impórtalo manualmente.')
    with tempfile.TemporaryDirectory(dir=base(api),prefix='texture-') as tmp:
        path=Path(tmp)/'pack.zip';path.write_bytes(raw);packs,errors=scan(path)
        if not packs:raise ValueError('El pack no es compatible con este importador. '+next(iter(errors),''))
        save_library(api.ROOT/'data/texture_library',packs)
    item['local']=True;item['digests']=[p.digest for p in packs];write(api,source,data)
    return {'media':data}
