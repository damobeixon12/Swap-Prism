"""Start Rocket League directly or through Steam as a non-Steam shortcut to its own .exe.

Steam's shortcuts.vdf is binary KeyValues: 0x00 map, 0x01 string, 0x02 int32, 0x08 end of map.
"""
from pathlib import Path
import os,shutil,struct,subprocess,time,zlib

NAME='Rocket League'


def exe(game):
    path=Path(game)/'Binaries/Win64/RocketLeague.exe'
    if not path.is_file():raise ValueError('No se encuentra RocketLeague.exe en la carpeta del juego.')
    return path


def play(game):
    path=exe(game);subprocess.Popen([str(path)],cwd=str(path.parent),creationflags=0x00000008)  # DETACHED_PROCESS
    return {'started':'direct'}


def steam_path():
    import winreg
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER,r'Software\Valve\Steam') as key:path=Path(winreg.QueryValueEx(key,'SteamPath')[0])
    except OSError:return None
    return path if (path/'steam.exe').is_file() else None


def steam_running():
    out=subprocess.run(['tasklist','/FI','IMAGENAME eq steam.exe','/NH'],capture_output=True,text=True,creationflags=0x08000000).stdout
    return 'steam.exe' in out.lower()


def _read(data,pos=0):
    out={}
    while True:
        kind=data[pos];pos+=1
        if kind==8:return out,pos
        end=data.index(0,pos);key=data[pos:end].decode('utf-8');pos=end+1
        if kind==0:out[key],pos=_read(data,pos)
        elif kind==1:end=data.index(0,pos);out[key]=data[pos:end].decode('utf-8');pos=end+1
        elif kind==2:out[key]=struct.unpack_from('<i',data,pos)[0];pos+=4
        else:raise ValueError('shortcuts.vdf con un tipo no reconocido.')


def _write(value):
    out=bytearray()
    for key,item in value.items():
        name=key.encode('utf-8')+b'\0'
        if isinstance(item,dict):out+=b'\0'+name+_write(item)
        elif isinstance(item,int):out+=b'\2'+name+struct.pack('<i',item)
        else:out+=b'\1'+name+str(item).encode('utf-8')+b'\0'
    return bytes(out+b'\x08')


def shortcut_id(target,name=NAME):
    """Steam's id for a non-Steam shortcut: crc32(exe+name) with the top bit set."""
    return zlib.crc32((target+name).encode('utf-8'))|0x80000000


def _files(steam):
    return [d/'config/shortcuts.vdf' for d in sorted((steam/'userdata').glob('*')) if d.name.isdigit() and d.name!='0']


def add_to_steam(root,game):
    steam=steam_path()
    if steam is None:raise ValueError('Steam no está instalado.')
    if steam_running():raise ValueError('Cierra Steam por completo (también en la bandeja) y vuelve a pulsar.')
    path=exe(game);target='"'+str(path)+'"';appid=shortcut_id(target)
    files=_files(steam)
    if not files:raise ValueError('Inicia sesión en Steam una vez antes de añadir el juego.')
    backup=Path(root)/'data/steam_backups'/time.strftime('%Y%m%d-%H%M%S')
    for vdf in files:
        data=_read(vdf.read_bytes())[0].get('shortcuts',{}) if vdf.exists() else {}
        if any(isinstance(e,dict) and e.get('Exe','').casefold()==target.casefold() for e in data.values()):continue
        if vdf.exists():backup.mkdir(parents=True,exist_ok=True);shutil.copy2(vdf,backup/(vdf.parent.parent.name+'.vdf'))
        data[str(len(data))]={'appid':struct.unpack('<i',struct.pack('<I',appid))[0],'AppName':NAME,'Exe':target,'StartDir':'"'+str(path.parent)+'"',
                              'icon':str(path),'ShortcutPath':'','LaunchOptions':'','IsHidden':0,'AllowDesktopConfig':1,'AllowOverlay':1,
                              'OpenVR':0,'Devkit':0,'DevkitGameID':'','DevkitOverrideAppID':0,'LastPlayTime':0,'FlatpakAppID':'','tags':{}}
        vdf.parent.mkdir(parents=True,exist_ok=True);raw=b'\0shortcuts\0'+_write(data)+b'\x08'
        if _read(raw)[0]['shortcuts']!=data:raise ValueError('No se pudo verificar shortcuts.vdf.')
        tmp=vdf.with_suffix('.swapprism.tmp');tmp.write_bytes(raw);os.replace(tmp,vdf)
    return status(game)


def status(game):
    steam=steam_path();result={'steam':steam is not None,'in_steam':False}
    if steam is None or not game:return result
    try:target='"'+str(exe(game))+'"'
    except ValueError:return result
    for vdf in _files(steam):
        if vdf.exists() and any(isinstance(e,dict) and e.get('Exe','').casefold()==target.casefold() for e in _read(vdf.read_bytes())[0].get('shortcuts',{}).values()):
            result['in_steam']=True
    return result


def play_steam(root,game):
    if not status(game)['in_steam']:add_to_steam(root,game)
    gameid=(shortcut_id('"'+str(exe(game))+'"')<<32)|0x02000000
    os.startfile(f'steam://rungameid/{gameid}')
    return {'started':'steam'}
