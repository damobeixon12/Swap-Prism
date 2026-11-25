from pathlib import Path
import json
import tempfile
import unittest
from unittest.mock import patch
import swap_manager as swaps
from menu_guard import EditState


class InstallRestore(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.root=Path(self.temp.name)
        self.cooked=self.root/'CookedPCConsole';self.cooked.mkdir()
        self.job=self.root/'job';(self.job/'generado').mkdir(parents=True)
        self.state=self.root/'data'
        self.name='explosion_test_SF.upk'
        self.target=self.cooked/self.name
        self.target.write_bytes(b'original-package')
        self.generated=self.job/'generado'/self.name
        self.generated.write_bytes(b'validated-replacement')
        self.report={'target':{'ID':1,'Product':'Original','AssetPackage':self.name},
                     'donor':{'ID':2,'Product':'Visual','AssetPackage':'donor.upk'},
                     'validation':{'serialized_payloads_unchanged':True},
                     'original_sha256':{self.name:swaps.sha(self.target)},
                     'generated_sha256':swaps.sha(self.generated)}
        (self.job/'informe.json').write_text(json.dumps(self.report))
        self.proc=patch.object(swaps,'require_editable',return_value=EditState('closed','test'))
        self.proc.start()

    def tearDown(self):
        self.proc.stop();self.temp.cleanup()

    def test_apply_then_restore_keeps_verified_original(self):
        swaps.apply_job(self.job,self.cooked,self.state)
        self.assertEqual(self.target.read_bytes(),b'validated-replacement')
        self.assertEqual(swaps.pristine(self.cooked,self.name,self.state).read_bytes(),b'original-package')
        self.assertEqual(swaps.restore_all(self.state),1)
        self.assertEqual(self.target.read_bytes(),b'original-package')

    def test_running_game_blocks_any_install(self):
        with patch.object(swaps,'require_editable',side_effect=ValueError('Partida')):
            with self.assertRaises(ValueError):swaps.apply_job(self.job,self.cooked,self.state)
        self.assertEqual(self.target.read_bytes(),b'original-package')
        self.assertFalse(self.state.exists())

    def test_menu_install_records_unconfirmed_reload(self):
        with patch.object(swaps,'require_editable',return_value=EditState('menu','test')):
            entry=swaps.apply_job(self.job,self.cooked,self.state)
            self.assertEqual(entry['reload_status'],'pending-garage-refresh')
            self.assertEqual(swaps.restore_all(self.state),1)

    def test_transition_before_write_never_changes_game_file(self):
        with patch.object(swaps,'require_editable',side_effect=[EditState('menu','test'),ValueError('Carga')]):
            with self.assertRaises(ValueError): swaps.apply_job(self.job,self.cooked,self.state)
        self.assertEqual(self.target.read_bytes(),b'original-package')

    def test_tampered_generated_package_is_rejected(self):
        self.generated.write_bytes(b'changed')
        with self.assertRaises(ValueError):swaps.apply_job(self.job,self.cooked,self.state)
        self.assertEqual(self.target.read_bytes(),b'original-package')

    def test_external_update_is_not_overwritten_by_restore(self):
        swaps.apply_job(self.job,self.cooked,self.state)
        self.target.write_bytes(b'game-update')
        with self.assertRaises(ValueError):swaps.restore_all(self.state)
        self.assertEqual(self.target.read_bytes(),b'game-update')

    def test_unvalidated_output_is_rejected(self):
        self.report['validation']={}
        (self.job/'informe.json').write_text(json.dumps(self.report))
        with self.assertRaises(ValueError):swaps.apply_job(self.job,self.cooked,self.state)
        self.assertFalse(self.state.exists())

    def test_meter_display_validation_rejects_gameplay_method_or_changed_amount(self):
        name='GFX_Hud_SF.upk'
        self.target=self.target.rename(self.cooked/name)
        self.generated=self.generated.rename(self.job/'generado'/name)
        self.report['target']['AssetPackage']=name
        self.report['original_sha256']={name:swaps.sha(self.target)}
        for methods,amount in [(['updateBoost','onBoostChanged','setBoostAmount'],True),(['updateBoost','onBoostChanged','onTeamColorChanged'],False)]:
            self.report['kind']='texture'
            self.report['validation']={'other_payloads_unchanged':True,'texture_and_hud_display_only':True,
                'hud_display_color':{'only_meter_display_methods_changed':True,'label_and_number_textColor_explicit':True,'methods':methods,'boost_amount_calculation_instructions_unchanged':amount}}
            (self.job/'informe.json').write_text(json.dumps(self.report))
            with self.assertRaises(ValueError):swaps.apply_job(self.job,self.cooked,self.state)
            self.assertEqual(self.target.read_bytes(),b'original-package')

    def test_restore_selected_leaves_other_installed_package_untouched(self):
        swaps.apply_job(self.job,self.cooked,self.state)
        state=swaps.load_state(self.state)
        other=self.cooked/'other.upk';other.write_bytes(b'keep-installed')
        state['other.upk']={'target':str(other),'status':'installed'}
        swaps.write_state(self.state,state)
        self.assertEqual(swaps.restore_all(self.state,names=[self.name]),1)
        self.assertEqual(self.target.read_bytes(),b'original-package')
        self.assertEqual(other.read_bytes(),b'keep-installed')
        self.assertEqual(swaps.load_state(self.state)['other.upk']['status'],'installed')

    def test_unknown_selection_never_restores_any_package(self):
        swaps.apply_job(self.job,self.cooked,self.state)
        with self.assertRaises(ValueError):swaps.restore_all(self.state,names=['missing.upk'])
        self.assertEqual(self.target.read_bytes(),b'validated-replacement')


    def test_plain_texture_accepts_explicit_null_hud_validation(self):
        self.report['kind']='texture'
        self.report['validation']={'other_payloads_unchanged':True,'texture_and_material_bindings_only':True,'hud_display_color':None}
        (self.job/'informe.json').write_text(json.dumps(self.report))
        swaps.apply_job(self.job,self.cooked,self.state)
        self.assertEqual(self.target.read_bytes(),b'validated-replacement')

    def test_hash_inspection_does_not_rewrite_install_history(self):
        swaps.apply_job(self.job,self.cooked,self.state)
        previous=(self.state/'swaps.json').read_bytes()
        self.target.write_bytes(b'original-package')
        view=swaps.inspect_state(self.state)
        self.assertEqual(view[self.name]['observed_status'],'original')
        self.assertEqual((self.state/'swaps.json').read_bytes(),previous)

    def test_profile_does_not_make_a_restored_cosmetic_look_installed(self):
        swaps.apply_job(self.job,self.cooked,self.state)
        entry=swaps.load_state(self.state)[self.name]
        (self.state/'profile.json').write_text(json.dumps({'packages':{self.name:{'installed_sha256':entry['installed_sha256'],'base_sha256':entry['original_sha256']}}}))
        self.assertEqual(swaps.inspect_state(self.state)[self.name]['observed_status'],'original')
        self.target.write_bytes(b'changed-by-something-else')
        self.assertEqual(swaps.inspect_state(self.state)[self.name]['observed_status'],'external')

if __name__=='__main__':unittest.main()


class EpicManifestBaseline(unittest.TestCase):
    """Bad backups and files changed by other programs, judged against Epic's manifest."""
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();root=Path(self.temp.name)
        self.cooked=root/'TAGame'/'CookedPCConsole';self.cooked.mkdir(parents=True)
        self.state=root/'data';self.name='explosion_test_SF.upk';self.target=self.cooked/self.name
        self.target.write_bytes(b'epic-original')
        self.epic=patch('epic_manifest.expected',return_value=swaps.epic_manifest.sha1(self.target));self.epic.start()
        self.proc=patch.object(swaps,'require_editable',return_value=EditState('closed','test'));self.proc.start()

    def tearDown(self):
        self.epic.stop();self.proc.stop();self.temp.cleanup()

    def bad_backup_state(self):
        backup=self.state/'backups'/self.name;backup.parent.mkdir(parents=True);backup.write_bytes(b'modified-by-old-version')
        swaps.write_state(self.state,{self.name:{'target':str(self.target),'backup':str(backup),'original_sha256':swaps.sha(backup),
                                                 'installed_sha256':'x','status':'restored'}})
        return backup

    def test_verified_file_heals_a_bad_backup(self):
        backup=self.bad_backup_state()
        self.assertEqual(swaps.pristine(self.cooked,self.name,self.state),self.target.resolve())
        self.assertEqual(backup.read_bytes(),b'epic-original')
        self.assertEqual(swaps.load_state(self.state)[self.name]['original_sha256'],swaps.sha(self.target))

    def test_untracked_modified_file_is_never_backed_up(self):
        self.target.write_bytes(b'changed-by-another-app')
        with self.assertRaisesRegex(ValueError,'Verificar archivos'):swaps.pristine(self.cooked,self.name,self.state)
        self.assertFalse(self.state.exists())

    def test_restore_accepts_file_already_reset_by_launcher(self):
        self.bad_backup_state();state=swaps.load_state(self.state);state[self.name]['status']='installed';swaps.write_state(self.state,state)
        self.assertEqual(swaps.restore_all(self.state),0)
        self.assertEqual(self.target.read_bytes(),b'epic-original')
