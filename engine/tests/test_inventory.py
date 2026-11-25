from pathlib import Path
import unittest
from player_inventory import extract_snapshot, instance_key


def fixture():
    objects = [
        {'__type': 'TAGame.OnlineProduct_TA', 'ProductID': 100, 'InstanceID': {'UpperBits': 5, 'LowerBits': 6}, 'Attributes': [1]},
        {'__type': 'TAGame.ProductAttribute_Painted_TA', 'PaintID': 12},
        {'__type': 'TAGame.OnlineProduct_TA', 'ProductID': 200, 'InstanceID': {'LowerBits': 7}, 'Attributes': [3]},
        {'__type': 'TAGame.ProductAttribute_Blueprint_TA', 'ProductID': 300},
        {'__type': 'TAGame.ProfileLoadoutSave_TA', 'LoadoutSets': [5], 'EquippedLoadoutSet': 5},
        {'__type': 'TAGame.LoadoutSet_TA', 'LoadoutSetName': 'Saved', 'Loadouts': [6, 7]},
        {'__type': 'TAGame.Loadout_TA', 'Products': [23, 100], 'OnlineProducts128': [{}, {'UpperBits': 5, 'LowerBits': 6}]},
        {'__type': 'TAGame.Loadout_TA', 'Products': [23, 0], 'OnlineProducts128': [{}, {}]},
    ]
    return {'file_info': {'crc_match': True}, 'objects': objects,
            'object_types': [{'object_index': index} for index in range(len(objects))],
            'properties': {'OnlineProducts': [0, 2]}}


class SavedInventory(unittest.TestCase):
    def setUp(self):
        self.database = [{'ID': 23, 'Product': 'Octane', 'Slot': 'Body'},
                         {'ID': 100, 'Product': 'Octane: Test', 'Slot': 'Decal'}]

    def test_zero_based_refs_and_active_preset(self):
        snapshot = extract_snapshot(fixture(), self.database, Path('fake.save'), 0)
        self.assertEqual(snapshot.active_index, 0)
        self.assertEqual(snapshot.active['teams'][0]['items'][0]['Product'], 'Octane')
        self.assertEqual(snapshot.active['teams'][0]['items'][1]['paint'], 'Titanium White')
        self.assertEqual(len(snapshot.active['teams']), 2)

    def test_blueprints_are_not_treated_as_unlocked(self):
        snapshot = extract_snapshot(fixture(), self.database, Path('fake.save'), 0)
        self.assertEqual(sum(item['blueprint'] for item in snapshot.inventory), 1)
        self.assertTrue(next(item for item in snapshot.inventory if item['ID'] == 200)['blueprint'])

    def test_no_silent_fallback_to_first_preset(self):
        data = fixture()
        data['objects'][4]['EquippedLoadoutSet'] = 999
        snapshot = extract_snapshot(data, self.database, Path('fake.save'), 0)
        self.assertIsNone(snapshot.active)

    def test_corrupt_crc_and_broken_references_fail(self):
        data = fixture()
        data['file_info']['crc_match'] = False
        with self.assertRaises(ValueError):
            extract_snapshot(data, self.database, Path('fake.save'), 0)
        data = fixture()
        data['properties']['OnlineProducts'].append(999)
        with self.assertRaises(ValueError):
            extract_snapshot(data, self.database, Path('fake.save'), 0)

    def test_missing_instance_bits_default_to_zero(self):
        self.assertEqual(instance_key({'LowerBits': 6}), (0, 6))

    def test_process_enumeration_can_identify_current_process(self):
        import os
        import sys
        if os.name != 'nt':
            self.skipTest('Windows only')
        from game_presence import process_ids
        self.assertIn(os.getpid(), process_ids(Path(sys.executable).name))


if __name__ == '__main__':
    unittest.main()
