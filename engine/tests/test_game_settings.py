import struct,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
from cryptography.hazmat.primitives.ciphers import Cipher,algorithms,modes
import game_settings as g, save_reader as sr

def prop(name,tag,raw,size):return sr.write_ue3(name)+sr.write_ue3(tag)+struct.pack('<ii',size,0)+raw

def fake_save():
    camera=sr.write_ue3('ProfileCameraSettings')
    inner=prop('FOV','FloatProperty',struct.pack('<f',100.),4)+sr.write_ue3('None')
    blob=prop('Camera','StructProperty',camera+inner,len(inner))+prop('bEnableCameraShake','BoolProperty',b'\x00',0)+sr.write_ue3('None')
    other=prop('MaxFPS','IntProperty',struct.pack('<i',144),4)+sr.write_ue3('None')
    data=struct.pack('<I',0xFFFFFFFF)+sr.write_ue3('None')
    cam_pos=len(data)+4;data+=struct.pack('<I',0xFFFFFFFF)+blob
    vid_pos=len(data)+4;data+=struct.pack('<I',0xFFFFFFFF)+other
    plain=bytes(20)+struct.pack('<i',len(data)+4)+data+struct.pack('<i',2)
    plain+=sr.write_ue3('TAGame.ProfileCameraSave_TA')+struct.pack('<II',cam_pos,0)+sr.write_ue3('TAGame.VideoSettingsSavePC_TA')+struct.pack('<II',vid_pos,1)
    plain+=bytes(-len(plain)%16)
    enc=Cipher(algorithms.AES(sr.AES_KEY),modes.ECB()).encryptor();body=enc.update(plain)+enc.finalize()
    return struct.pack('<II',len(body),sr.crc32(body))+body

class GameSettingsSave(unittest.TestCase):
    def test_roundtrip_is_byte_exact(self):
        raw=fake_save();self.assertEqual(g.Save(raw).encode(),raw)

    def test_insert_missing_value_keeps_other_objects(self):
        save=g.Save(fake_save())
        g.put(save,'camera.distance',280);g.put(save,'camera.fov',109);g.put(save,'camera.shake',True)
        again=g.Save(save.encode())
        self.assertEqual(g.get(again,'camera.distance'),280.);self.assertEqual(g.get(again,'camera.fov'),109.)
        self.assertTrue(g.get(again,'camera.shake'));self.assertEqual(g.get(again,'video.max_fps'),144)

    def test_write_keeps_backup_and_refuses_open_game(self):
        with tempfile.TemporaryDirectory() as tmp:
            base=Path(tmp)/'TAGame';save=base/'SaveDataEpic/X/a.save';save.parent.mkdir(parents=True);save.write_bytes(fake_save())
            with patch('menu_guard.require_closed',side_effect=ValueError('Partida')):
                with self.assertRaises(ValueError):g.write(tmp,{'camera.fov':90},base)
            self.assertEqual(save.read_bytes(),fake_save())
            with patch('menu_guard.require_closed'):g.write(tmp,{'camera.fov':90},base)
            self.assertEqual(g.get(g.Save(save.read_bytes()),'camera.fov'),90.)
            self.assertEqual(next((Path(tmp)/'data/settings_backups').glob('*/a.save')).read_bytes(),fake_save())
