"""Camera, controller and video settings, edited in the local save without opening the game.

Save layout (see save_reader): [len][crc][AES-ECB: 24-byte header, savedata, object table, zero padding].
Edits rewrite one tagged property (or insert it when the game left it at its default),
then fix the enclosing struct sizes, the object table and the CRC. Every write keeps a backup.
"""
from pathlib import Path
import os,shutil,struct,time
from cryptography.hazmat.primitives.ciphers import Cipher,algorithms,modes
import save_reader as sr

# (object type, path inside it, property type) — names checked against TAGame.upk.
FIELDS={
    'camera.fov':('TAGame.ProfileCameraSave_TA',('Camera','FOV'),'FloatProperty'),
    'camera.distance':('TAGame.ProfileCameraSave_TA',('Camera','Distance'),'FloatProperty'),
    'camera.height':('TAGame.ProfileCameraSave_TA',('Camera','Height'),'FloatProperty'),
    'camera.pitch':('TAGame.ProfileCameraSave_TA',('Camera','Pitch'),'FloatProperty'),
    'camera.stiffness':('TAGame.ProfileCameraSave_TA',('Camera','Stiffness'),'FloatProperty'),
    'camera.swivel':('TAGame.ProfileCameraSave_TA',('Camera','SwivelSpeed'),'FloatProperty'),
    'camera.transition':('TAGame.ProfileCameraSave_TA',('Camera','TransitionSpeed'),'FloatProperty'),
    'camera.shake':('TAGame.ProfileCameraSave_TA',('bEnableCameraShake',),'BoolProperty'),
    'camera.invert_swivel':('TAGame.ProfileCameraSave_TA',('bInvertSwivelPitch',),'BoolProperty'),
    'controls.deadzone':('TAGame.ProfileGamepadSave_TA',('ControllerDeadzone',),'FloatProperty'),
    'controls.dodge_deadzone':('TAGame.ProfileGamepadSave_TA',('DodgeInputThreshold',),'FloatProperty'),
    'controls.steering':('TAGame.ProfileGamepadSave_TA',('SteeringSensitivity',),'FloatProperty'),
    'controls.aerial':('TAGame.ProfileGamepadSave_TA',('AirControlSensitivity',),'FloatProperty'),
    'controls.vibration':('TAGame.ProfileControlsSave_TA',('ForceFeedbackScale',),'FloatProperty'),
    'video.max_fps':('TAGame.VideoSettingsSavePC_TA',('MaxFPS',),'IntProperty'),
    'video.weather':('TAGame.VideoSettingsSavePC_TA',('bShowWeatherFX',),'BoolProperty'),
    'video.light_shafts':('TAGame.VideoSettingsSavePC_TA',('bShowLightShafts',),'BoolProperty'),
    'video.lens_flares':('TAGame.VideoSettingsSavePC_TA',('bShowLensFlares',),'BoolProperty'),
}
# System ini keys, [SystemSettings] only.
INI_FIELDS={'video.res_x':'ResX','video.res_y':'ResY','video.fullscreen':'Fullscreen','video.borderless':'Borderless','video.vsync':'UseVsync'}


def folder():
    return Path(os.path.expanduser('~'))/'Documents/My Games/Rocket League/TAGame'


def saves(base=None):
    """Every copy of the profile save (the game keeps a second '_2' copy)."""
    base=Path(base or folder())
    found=[p for d in ('SaveDataEpic','SaveData') for p in (base/d).glob('*/*.save')]
    return sorted(found,key=lambda p:p.stat().st_mtime,reverse=True)


class Save:
    def __init__(self,raw):
        size=struct.unpack_from('<I',raw,0)[0]
        if struct.unpack_from('<I',raw,4)[0]!=sr.crc32(raw[8:8+size]):raise ValueError('La partida guardada está dañada (CRC).')
        dec=sr.aes_decrypt(raw[8:8+size]);self.head=dec[:20]
        svlen=struct.unpack_from('<i',dec,20)[0];self.data=bytearray(dec[24:24+svlen-4]);off=24+svlen-4
        count=struct.unpack_from('<i',dec,off)[0];off+=4;self.table=[]
        for _ in range(count):
            name,off=sr.read_ue3(dec,off);pos,index=struct.unpack_from('<II',dec,off);off+=8;self.table.append([name,pos,index])
        if any(dec[off:]):raise ValueError('Formato de partida guardada no reconocido.')

    def encode(self):
        out=bytearray(self.head)+struct.pack('<i',len(self.data)+4)+self.data+struct.pack('<i',len(self.table))
        for name,pos,index in self.table:out+=sr.write_ue3(name)+struct.pack('<II',pos,index)
        out+=bytes(-len(out)%16)
        enc=Cipher(algorithms.AES(sr.AES_KEY),modes.ECB()).encryptor();body=enc.update(bytes(out))+enc.finalize()
        return struct.pack('<II',len(body),sr.crc32(body))+body

    def span(self,kind):
        rows=[i for i,row in enumerate(self.table) if row[0]==kind]
        if len(rows)!=1:raise ValueError('No se encuentra '+kind+' en la partida guardada.')
        i=rows[0];start=self.table[i][1]-4+4
        end=self.table[i+1][1]-4 if i+1<len(self.table) else len(self.data)
        return i,start,end


def _walk(buf,off):
    """Tagged properties with their offsets; stops after 'None'. Returns (entries, none_start, end)."""
    entries=[]
    while True:
        start=off;name,off=sr.read_ue3(buf,off)
        if name=='None':return entries,start,off
        tag,off=sr.read_ue3(buf,off);size_at=off;size=struct.unpack_from('<i',buf,off)[0];off+=8;value_at=off
        entry={'name':name,'tag':tag,'size_at':size_at,'value_at':value_at}
        if tag=='StructProperty':
            _,inner=sr.read_ue3(buf,off);entry['children'],entry['none_at'],off=_walk(buf,inner)
        else:
            _,off=sr._parse_value(bytes(buf),off,tag,size)
        entries.append(entry)


def _encode(tag,value):
    if tag=='FloatProperty':return struct.pack('<f',float(value)),4
    if tag=='IntProperty':return struct.pack('<i',int(value)),4
    if tag=='BoolProperty':return bytes([1 if value else 0]),0
    raise ValueError('Tipo no editable: '+tag)


def get(save,key):
    kind,path,tag=FIELDS[key];_,start,end=save.span(kind);blob=bytes(save.data[start:end])
    entries,_,_=_walk(blob,0)
    for name in path:
        entry=next((e for e in entries if e['name']==name),None)
        if entry is None:return None
        entries=entry.get('children',[])
    value,_=sr._parse_value(blob,entry['value_at'],tag,4)
    return value


def put(save,key,value):
    kind,path,tag=FIELDS[key];index,start,end=save.span(kind);blob=bytearray(save.data[start:end])
    raw,size=_encode(tag,value)
    entries,none_at,_=_walk(bytes(blob),0);parents=[]
    for depth,name in enumerate(path):
        entry=next((e for e in entries if e['name']==name),None)
        if entry is None:
            if depth!=len(path)-1:raise ValueError('Falta la estructura '+name)
            insert=sr.write_ue3(name)+sr.write_ue3(tag)+struct.pack('<ii',size,0)+raw
            blob[none_at:none_at]=insert
            for parent in parents:struct.pack_into('<i',blob,parent['size_at'],struct.unpack_from('<i',blob,parent['size_at'])[0]+len(insert))
            break
        if depth==len(path)-1:
            blob[entry['value_at']:entry['value_at']+len(raw)]=raw;break
        parents.append(entry);entries,none_at=entry['children'],entry['none_at']
    delta=len(blob)-(end-start)
    save.data[start:end]=blob
    if delta:
        for row in save.table[index+1:]:row[1]+=delta


def _ini_path(base=None):return Path(base or folder())/'Config/TASystemSettings.ini'


def _ini_section(text):
    """Bounds of the first [SystemSettings] section."""
    start=text.index('[SystemSettings]')
    nxt=text.find('\n[',start+1)
    return start,(len(text) if nxt<0 else nxt+1)


def read(base=None):
    files=saves(base)
    if not files:raise ValueError('No se encuentra la partida guardada de Rocket League.')
    save=Save(files[0].read_bytes());present={row[0] for row in save.table}
    values={key:get(save,key) if FIELDS[key][0] in present else None for key in FIELDS}
    skills=[]
    if 'TAGame.PlaylistSkillDataSave_TA' in present:
        _,start,end=save.span('TAGame.PlaylistSkillDataSave_TA')
        skills=[s for s in sr.parse_property_stream(bytes(save.data[start:end]),0)[0].get('SkillData',[]) if isinstance(s,dict)]
    ini=_ini_path(base)
    if ini.exists():
        text=ini.read_bytes().decode('utf-8','replace');a,b=_ini_section(text)
        for key,name in INI_FIELDS.items():
            line=next((l for l in text[a:b].splitlines() if l.split('=')[0].strip()==name),None)
            values[key]=None if line is None else line.split('=',1)[1].strip()
    return {'values':values,'skills':skills,'save':str(files[0])}


def write(root,changes,base=None):
    from menu_guard import require_closed
    require_closed()
    unknown=set(changes)-set(FIELDS)-set(INI_FIELDS)
    if unknown:raise ValueError('Ajuste no válido: '+', '.join(sorted(unknown)))
    files=saves(base)
    if not files:raise ValueError('No se encuentra la partida guardada de Rocket League.')
    backup=Path(root)/'data/settings_backups'/time.strftime('%Y%m%d-%H%M%S');backup.mkdir(parents=True,exist_ok=True)
    save_changes={k:v for k,v in changes.items() if k in FIELDS}
    for path in files:
        raw=path.read_bytes();shutil.copy2(path,backup/path.name)
        save=Save(raw)
        for key,value in save_changes.items():put(save,key,value)
        out=save.encode();check=Save(out)
        for key,value in save_changes.items():
            got=get(check,key)
            if got is None or (abs(float(got)-float(value))>1e-4 if FIELDS[key][2]!='BoolProperty' else bool(got)!=bool(value)):
                raise ValueError('No se pudo verificar '+key+'. No se ha cambiado nada.')
        tmp=path.with_suffix('.swapprism.tmp');tmp.write_bytes(out);require_closed();os.replace(tmp,path)
    ini_changes={INI_FIELDS[k]:v for k,v in changes.items() if k in INI_FIELDS}
    ini=_ini_path(base)
    if ini_changes and ini.exists():
        shutil.copy2(ini,backup/ini.name);text=ini.read_bytes().decode('utf-8','replace');a,b=_ini_section(text)
        lines=text[a:b].split('\n')
        for name,value in ini_changes.items():
            value=('True' if value else 'False') if isinstance(value,bool) else str(int(value))
            at=next((i for i,l in enumerate(lines) if l.split('=')[0].strip()==name),None)
            if at is None:lines.insert(1,f'{name}={value}')
            else:lines[at]=f'{name}={value}'+('\r' if lines[at].endswith('\r') else '')
        tmp=ini.with_suffix('.swapprism.tmp');tmp.write_text(text[:a]+'\n'.join(lines)+text[b:],encoding='utf-8',newline='');os.replace(tmp,ini)
    return {**read(base),'backup':str(backup)}
