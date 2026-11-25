"""Original SHA-1 of every game file, read from Epic's .egstore/*.manifest (read-only)."""
from pathlib import Path
import hashlib,struct,zlib

MAGIC=0x44BEC00C


class _Reader:
    def __init__(self,data):self.data,self.pos=data,0
    def take(self,n):
        if self.pos+n>len(self.data):raise ValueError('Manifiesto de Epic truncado.')
        chunk=self.data[self.pos:self.pos+n];self.pos+=n;return chunk
    def u8(self):return self.take(1)[0]
    def u32(self):return struct.unpack('<I',self.take(4))[0]
    def i32(self):return struct.unpack('<i',self.take(4))[0]
    def fstring(self):
        size=self.i32()
        if size==0:return ''
        if size<0:return self.take(-size*2).decode('utf-16-le').rstrip('\0')
        return self.take(size).decode('ascii','replace').rstrip('\0')


def parse(path):
    """Return {relative path (forward slashes, casefolded): sha1 hex}."""
    raw=Path(path).read_bytes();head=_Reader(raw)
    if head.u32()!=MAGIC:raise ValueError('No es un manifiesto binario de Epic.')
    header_size=head.u32();head.u32();head.u32();head.take(20);stored_as=head.u8()
    body=raw[header_size:]
    if stored_as&1:body=zlib.decompress(body)
    r=_Reader(body)
    start=r.pos;r.pos=start+r.u32()          # ManifestMeta
    start=r.pos;r.pos=start+r.u32()          # ChunkDataList
    r.u32();r.u8();count=r.u32()             # FileManifestList: size, version, count
    names=[r.fstring() for _ in range(count)]
    for _ in range(count):r.fstring()        # symlink targets
    hashes=[r.take(20).hex() for _ in range(count)]
    return {name.replace('\\','/').casefold():digest for name,digest in zip(names,hashes)}


def find(game):
    """Epic manifest for this install, or None (Steam or missing .egstore)."""
    folder=Path(game)/'.egstore'
    manifests=sorted(folder.glob('*.manifest'),key=lambda p:p.stat().st_mtime) if folder.is_dir() else []
    return manifests[-1] if manifests else None


_cache={}

def expected(game,name):
    """Epic's SHA-1 for a CookedPCConsole package, or None when there is no Epic manifest."""
    manifest=find(game)
    if manifest is None:return None
    key=(str(manifest),manifest.stat().st_mtime)
    if key not in _cache:_cache.clear();_cache[key]=parse(manifest)
    return _cache[key].get(('TAGame/CookedPCConsole/'+name).casefold())


def sha1(path):
    digest=hashlib.sha1()
    with Path(path).open('rb') as stream:
        for block in iter(lambda:stream.read(1024*1024),b''):digest.update(block)
    return digest.hexdigest()


def check(game,names):
    """{package: 'original'|'modified'|'missing'|'unknown'} for files in TAGame/CookedPCConsole."""
    cooked=Path(game)/'TAGame/CookedPCConsole';result={}
    for name in names:
        target=cooked/name;want=expected(game,name)
        if not target.exists():result[name]='missing'
        elif want is None:result[name]='unknown'
        else:result[name]='original' if sha1(target)==want else 'modified'
    return result
