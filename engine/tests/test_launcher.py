import tempfile,unittest
from pathlib import Path
from unittest.mock import patch
import launcher

class EpicLaunch(unittest.TestCase):
    def test_link_for_matching_install(self):
        import json
        with tempfile.TemporaryDirectory() as tmp:
            dat=Path(tmp)/'LauncherInstalled.dat'
            dat.write_text(json.dumps({'InstallationList':[{'InstallLocation':tmp,'NamespaceId':'ns','ItemId':'it','AppName':'Sugar'}]}),encoding='utf-8')
            self.assertEqual(launcher.epic_url(tmp,dat),'com.epicgames.launcher://apps/ns%3Ait%3ASugar?action=launch&silent=true')
            with self.assertRaisesRegex(ValueError,'Steam'):launcher.epic_url(Path(tmp)/'otro',dat)

class SteamShortcut(unittest.TestCase):
    def test_vdf_roundtrip_and_known_id(self):
        data={'0':{'appid':-5,'AppName':'X','Exe':'"C:\a.exe"','IsHidden':0,'tags':{}}}
        raw=b'\0shortcuts\0'+launcher._write(data)+b'\x08'
        self.assertEqual(launcher._read(raw)[0]['shortcuts'],data)
        self.assertTrue(launcher.shortcut_id('"C:\a.exe"')&0x80000000)

    def test_add_once_keeps_existing_shortcuts(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp=Path(tmp);steam=tmp/'Steam';(steam/'userdata/123/config').mkdir(parents=True)
            other={'0':{'appid':1,'AppName':'Otro','Exe':'"C:\otro.exe"','tags':{}}}
            vdf=steam/'userdata/123/config/shortcuts.vdf';vdf.write_bytes(b'\0shortcuts\0'+launcher._write(other)+b'\x08')
            game=tmp/'rl';(game/'Binaries/Win64').mkdir(parents=True);(game/'Binaries/Win64/RocketLeague.exe').write_bytes(b'')
            with patch.object(launcher,'steam_path',return_value=steam),patch.object(launcher,'steam_running',return_value=False):
                launcher.add_to_steam(tmp,game);launcher.add_to_steam(tmp,game)
                self.assertTrue(launcher.status(game)['in_steam'])
            shortcuts=launcher._read(vdf.read_bytes())[0]['shortcuts']
            self.assertEqual(len(shortcuts),2);self.assertEqual(shortcuts['0']['AppName'],'Otro')

    def test_steam_open_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            with patch.object(launcher,'steam_path',return_value=Path(tmp)),patch.object(launcher,'steam_running',return_value=True):
                with self.assertRaisesRegex(ValueError,'Cierra Steam'):launcher.add_to_steam(tmp,tmp)
