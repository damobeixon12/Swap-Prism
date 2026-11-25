from pathlib import Path
import io,json,tempfile,unittest,zipfile
from unittest.mock import patch
from PIL import Image
from texture_library import read,scan,safe_member,save_library,load_library
from menu_guard import require_closed
from texture_engine import mip_table

class TextureImports(unittest.TestCase):
    def image(self):
        b=io.BytesIO();Image.new('RGBA',(8,8),(120,50,30,255)).save(b,format='PNG');return b.getvalue()
    def test_zip_and_unpacked_copy_are_deduplicated(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);raw=self.image();config=json.dumps({'My Ball':{'Params':{'Diffuse':'ball.png'}}})
            (root/'ball.png').write_bytes(raw);(root/'pack.json').write_text(config)
            with zipfile.ZipFile(root/'pack.zip','w') as z:z.writestr('pack.json',config);z.writestr('ball.png',raw)
            packs,errors=scan(root);self.assertEqual(errors,[]);self.assertEqual(len(packs),1)
            save_library(root/'library',packs);loaded=load_library(root/'library')
            self.assertEqual(loaded[0].layers,packs[0].layers)
            self.assertEqual(loaded[0].digest,packs[0].digest)
    def test_loose_png_is_a_boostmeter_background(self):
        with tempfile.TemporaryDirectory() as tmp:
            file=Path(tmp)/'meter.png';file.write_bytes(self.image());packs=read(file)
            self.assertEqual(packs[0].category,'Boostmeter')
            self.assertEqual(set(packs[0].layers),{'Background'})
    def test_archive_traversal_is_rejected_without_extracting(self):
        with tempfile.TemporaryDirectory() as tmp:
            file=Path(tmp)/'bad.zip'
            with zipfile.ZipFile(file,'w') as z:z.writestr('../evil.png',self.image())
            with self.assertRaises(ValueError):read(file)
    def test_config_cannot_reference_outside_pack(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);file=root/'bad.json';file.write_text(json.dumps({'Bad':{'Params':{'Diffuse':'../image.png'}}}))
            with self.assertRaises(ValueError):read(file)
    def test_missing_image_is_not_silently_skipped(self):
        with tempfile.TemporaryDirectory() as tmp:
            file=Path(tmp)/'bad.zip'
            with zipfile.ZipFile(file,'w') as z:z.writestr('ball.json',json.dumps({'Ball':{'Params':{'Diffuse':'missing.png'}}}))
            with self.assertRaises(ValueError):read(file)
    def test_invalid_mips_fail_closed(self):
        with self.assertRaises(ValueError):mip_table(bytes(200),0)
    def test_closed_game_required_even_in_menu(self):
        with patch('menu_guard.process_ids',return_value={123}):
            with self.assertRaises(ValueError):require_closed()

if __name__=='__main__':unittest.main()
