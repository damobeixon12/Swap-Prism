import json
import sys
from pathlib import Path
import tempfile
import unittest
import local_skins as app


class Boundaries(unittest.TestCase):
    def test_body_requires_a_resolvable_visual_asset(self):
        self.assertIn('Body', app.SLOTS)
        self.assertFalse(app.compatibility({'Slot': 'Body', 'Product': 'Octane'}, 'Octane')[0])
        self.assertTrue(app.compatibility({'Slot':'Body','Product':'Fennec','AssetPath':'body_grain.body_grain','AssetPackage':'body_grain_SF.upk'},'Octane')[0])

    def test_body_specific_decal_must_match_exactly(self):
        decal = {'Slot': 'Decal', 'Product': 'Octane: Flames'}
        self.assertTrue(app.compatibility(decal, 'Octane')[0])
        self.assertFalse(app.compatibility(decal, 'Fennec')[0])
        self.assertFalse(app.compatibility(decal, 'Octane ZSR')[0])

    def test_unknown_cars_and_restricted_items_are_blocked(self):
        wheel = {'Slot': 'Wheels', 'Product': 'Tunica'}
        self.assertTrue(app.compatibility(wheel, 'Octane')[0])
        self.assertFalse(app.compatibility(wheel, 'Batmobile')[0])
        self.assertFalse(app.compatibility({**wheel, 'Attributes': [{'Key': 'IsLicensed'}]}, 'Octane')[0])

    def test_goal_explosions_allowed_with_car_rules(self):
        self.assertIn('Goal Explosion', app.SLOTS)
        self.assertTrue(app.compatibility({'Slot': 'Goal Explosion', 'Product': 'Dueling Dragons'}, 'Octane')[0])
        self.assertFalse(app.compatibility({'Slot': 'Body', 'Product': 'Octane'}, 'Octane')[0])

    @unittest.skipUnless(sys.platform == 'win32', 'Comprueba procesos con la API de Windows.')
    def test_shared_packages_and_missing_confirmation_create_no_output(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            game, engine, output = base/'game', base/'engine', base/'output'
            cooked = game/'TAGame/CookedPCConsole'
            cooked.mkdir(parents=True)
            engine.mkdir()
            (cooked/'shared.upk').write_bytes(b'unchanged')
            rows = [dict(ID=n, Slot='Antenna', Product=str(n), AssetPackage='shared.upk', AssetPath='A.B') for n in (1, 2)]
            (engine/'items.json').write_text(json.dumps({'Items': rows}))
            for confirmed in (False, True):
                with self.subTest(confirmed=confirmed), self.assertRaises(ValueError):
                    app.generate(engine, game, output, 1, 2, car='Octane', compatibility_confirmed=confirmed)
                self.assertFalse(output.exists())
                self.assertEqual((cooked/'shared.upk').read_bytes(), b'unchanged')

    def test_reject_path_traversal(self):
        for value in ['../x.upk', '..\\x.upk', 'C:x.upk', 'x.dll', '/x.upk']:
            with self.subTest(value=value), self.assertRaises(ValueError):
                app.safe_package(value)

    def test_output_cannot_overlap_game_or_engine(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            for path in [base, base/'game', base/'game'/'TAGame', base/'engine', base/'engine'/'generated']:
                with self.subTest(path=path), self.assertRaises(ValueError):
                    app.separate_output(path, base/'game', base/'engine')
            self.assertEqual(app.separate_output(base/'output', base/'game', base/'engine'), (base/'output').resolve())

    @unittest.skipUnless(sys.platform == 'win32', 'Comprueba procesos con la API de Windows.')
    def test_wrong_category_never_creates_job(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            game, engine, output = base/'game', base/'engine', base/'output'
            cooked = game/'TAGame/CookedPCConsole'
            cooked.mkdir(parents=True)
            engine.mkdir()
            rows = []
            for number, slot in [(1, 'Antenna'), (2, 'Topper')]:
                name = str(number) + '.upk'
                (cooked/name).write_bytes(b'unchanged')
                rows.append(dict(ID=number, Slot=slot, Product=name, AssetPackage=name, AssetPath='A.B'))
            (engine/'items.json').write_text(json.dumps({'Items': rows}))
            with self.assertRaises(ValueError):
                app.generate(engine, game, output, 1, 2)
            self.assertFalse(output.exists())
            self.assertEqual((cooked/'1.upk').read_bytes(), b'unchanged')


if __name__ == '__main__':
    unittest.main()
