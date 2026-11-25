"""Verified catalog artwork cache. Bounded downloads, atomic writes, no fabricated art."""
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor,as_completed
from urllib.request import Request,urlopen
from urllib.parse import urlparse
from PIL import Image
import hashlib,io,os,threading,time

def cache_path(folder,url):
    return Path(folder)/(hashlib.sha256(url.encode()).hexdigest()+'.png')

def fetch(row,folder,attempts=2):
    url=row.get('Image')
    if not url or urlparse(url).scheme!='https' or urlparse(url).hostname!='cdn.source.gs':return None
    path=cache_path(folder,url)
    if path.is_file():return path
    path.parent.mkdir(parents=True,exist_ok=True)
    for attempt in range(attempts):
        try:
            with urlopen(Request(url,headers={'User-Agent':'LocalSkins/0.7'}),timeout=8) as response:
                if urlparse(response.geturl()).hostname!='cdn.source.gs':return None
                raw=response.read(8*1024*1024+1)
            if len(raw)>8*1024*1024:raise ValueError('Imagen demasiado grande')
            with Image.open(io.BytesIO(raw)) as source:
                if source.width*source.height>4096*4096:raise ValueError('Resolución demasiado grande')
                source=source.convert('RGBA');source.thumbnail((112,112),Image.Resampling.LANCZOS)
                encoded=io.BytesIO();source.save(encoded,format='PNG');raw=encoded.getvalue()
            temp=path.with_suffix(f'.{threading.get_ident()}.tmp');temp.write_bytes(raw);os.replace(temp,path)
            return path
        except Exception:
            if attempt+1<attempts:time.sleep(.2)
    return None

def download_all(rows,folder,progress=lambda *args:None):
    unique={r['Image']:r for r in rows if r.get('Image')}
    done=ok=0;failed=[]
    with ThreadPoolExecutor(max_workers=8) as pool:
        jobs={pool.submit(fetch,r,folder):r for r in unique.values()}
        for job in as_completed(jobs):
            row=jobs[job];result=job.result();done+=1
            if result:ok+=1
            else:failed.append(row['ID'])
            if done%100==0 or done==len(jobs):progress(done,len(jobs),ok)
    return {'total':len(unique),'cached':ok,'failed_ids':failed}
