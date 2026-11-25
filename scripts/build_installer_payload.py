"""Generate extraction instructions and byte-weighted completed-file progress."""
from pathlib import Path
root=Path.cwd();dist=root/'work/distribution';files=[]
for source,target in [(dist/'Swap Prism.exe','Swap Prism.exe'),(dist/'WebView2Loader.dll','WebView2Loader.dll'),(root/'app/assets/swapprism.ico','swapprism.ico'),(dist/'THIRD_PARTY.txt','THIRD_PARTY.txt')]:files.append((source,target,False))
files.extend((p,'_engine/'+p.relative_to(dist/'_engine').as_posix(),False) for p in sorted((dist/'_engine').rglob('*')) if p.is_file())
for name in ['PresentMon.exe','PresentMon-LICENSE.txt']:files.append((root/'app/vendor'/name,'_telemetry/'+name,False))
files.extend((p,'data/thumbnails/'+p.name,True) for p in sorted((root/'work/package-thumbnails').glob('*.webp')))
files.append((root/'app/assets/settings.json','data/settings.json',True))
total=sum(p.stat().st_size for p,_,_ in files);done=0;percent=-1;lines=[];last_dir=None;last_keep=None
def escape(s):return str(s).replace('$','$$').replace('"','$\\"')
for source,target,keep in files:
 parent=Path(target).parent.as_posix().replace('/','\\');parent='' if parent=='.' else '\\'+parent
 if parent!=last_dir:lines.append('SetOutPath "$INSTDIR'+parent+'"');last_dir=parent
 if keep!=last_keep:lines.append('SetOverwrite '+('off' if keep else 'on'));last_keep=keep
 lines.append('File "/oname='+escape(Path(target).name)+'" "'+escape(source)+'"')
 done+=source.stat().st_size;now=round(done/total*98)
 if now>percent:lines.append('!insertmacro UpdateProgress '+str(now));percent=now
lines.append('SetOverwrite on')
(root/'work/installer-payload.nsh').write_text('\n'.join(lines)+'\n',encoding='utf8')
print('Installer payload files',len(files),'unpacked bytes',total)
