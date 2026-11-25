import io,json,tempfile,unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
from PIL import Image
import profile_manager as profile
from fake_display import validate_name
from swap_manager import sha,load_state,write_state

class ProfileLayer(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.root=Path(self.tmp.name)
        self.game=self.root/'game';self.cooked=self.game/'TAGame/CookedPCConsole';self.cooked.mkdir(parents=True)
        self.api=SimpleNamespace(ROOT=self.root,DEFAULT_ENGINE=self.root,safe_package=lambda n:n)
    def tearDown(self):self.tmp.cleanup()

    def test_name_is_unicode_and_rejects_hidden_controls(self):
        self.assertEqual(validate_name('  Peñá 🚗  '),'Peñá 🚗')
        for name in ['x\ny','x\x00y','x\u202ey','x'*33]:
            with self.assertRaises(ValueError):validate_name(name)

    def test_import_copies_and_normalizes_images_without_changing_source(self):
        source=self.root/'original.png';Image.new('RGBA',(80,40),'red').save(source);before=source.read_bytes()
        target=Path(profile.import_image(self.root,source,'avatar'))
        self.assertEqual(source.read_bytes(),before)
        with Image.open(target) as im:self.assertEqual(im.size,(512,512))
        self.assertEqual(profile.normalize(self.root,{'name':'Pepe','avatar':str(target)})['name'],'Pepe')
        with self.assertRaises(ValueError):profile.normalize(self.root,{'avatar':str(source)})

    def test_stale_base_or_external_change_is_rejected(self):
        target=self.cooked/'GFX_Hud_SF.upk';target.write_bytes(b'profile');base=self.root/target.name;base.write_bytes(b'cosmetics')
        p={'packages':{target.name:{'base':str(base),'base_sha256':sha(base),'installed_sha256':sha(target)}}}
        self.assertEqual(profile._base(self.root,self.cooked,target.name,p),base)
        target.write_bytes(b'external')
        with self.assertRaises(ValueError):profile._base(self.root,self.cooked,target.name,p)
        target.write_bytes(b'profile');base.write_bytes(b'broken')
        with self.assertRaises(ValueError):profile._base(self.root,self.cooked,target.name,p)

    def test_profile_revert_restores_cosmetic_registry_instead_of_forcing_vanilla(self):
        name='GFX_Hud_SF.upk';base={'status':'installed','installed_sha256':'cosmetic-hash','donor_item':{'Product':'My boostmeter'}}
        job=self.root/'job';job.mkdir();(job/'informe.json').write_text(json.dumps({'target':{'AssetPackage':name},'generated_sha256':'cosmetic-hash','profile_base_state':base}))
        profile.save(self.root,{'schema':1,'settings':{'name':'Pepe'},'packages':{name:{'base_state':base}}})
        write_state(self.root/'data',{name:{'status':'installed','installed_sha256':'profile-hash','profile_layer':True}})
        new={'schema':1,'settings':{'name':'','avatar':'','banner':''},'packages':{}}
        with patch.object(profile,'require_closed'),patch.object(profile,'_generate',return_value=([job],new)),patch('change_queue.install_all',return_value=1):
            result=profile.apply(self.api,self.game,{'name':''})
        self.assertEqual(result['state'][name],base)
        self.assertEqual(profile.load(self.root)['packages'],{})

    def test_failed_install_does_not_commit_new_profile(self):
        original=profile.load(self.root);profile.save(self.root,original)
        with patch.object(profile,'require_closed'),patch.object(profile,'_generate',return_value=([],{'schema':1,'settings':{'name':'Other'},'packages':{}})),patch('change_queue.install_all',side_effect=ValueError('write failed')):
            with self.assertRaises(ValueError):profile.apply(self.api,self.game,{'name':'Other'})
        self.assertEqual(profile.load(self.root),original)

    def test_game_open_is_rejected_before_preparation(self):
        with patch.object(profile,'require_closed',side_effect=ValueError('Close Rocket League')),patch.object(profile,'_generate') as generate:
            with self.assertRaises(ValueError):profile.apply(self.api,self.game,{'name':'Pepe'})
            generate.assert_not_called()

    def test_restoring_one_package_marks_profile_partial(self):
        profile.save(self.root,{'schema':1,'settings':{'name':'Pepe'},'packages':{'a.upk':{},'b.upk':{}}})
        profile.after_restore(self.root,['a.upk']);value=profile.load(self.root)
        self.assertTrue(value['partial']);self.assertEqual(set(value['packages']),{'b.upk'})
        profile.after_restore(self.root,None);self.assertFalse(profile.load(self.root)['packages'])

    def test_unrelated_cosmetic_does_not_turn_vanilla_profile_bases_into_cosmetic_entries(self):
        value={'schema':1,'settings':{'name':'Pepe'},'packages':{'Startup.upk':{'base_state':None,'base_sha256':'vanilla'}}}
        profile.save(self.root,value)
        write_state(self.root/'data',{'Startup.upk':{'target_item':{'Slot':'Profile'},'installed_sha256':'profile'}})
        job=self.root/'job';job.mkdir();(job/'informe.json').write_text(json.dumps({'target':{'AssetPackage':'wheel.upk'}}))
        composed=profile.compose_jobs(self.api,[job]);profile.commit_composed(self.api,composed)
        self.assertIsNone(profile.load(self.root)['packages']['Startup.upk']['base_state'])
        self.assertNotIn('_composed',profile.load(self.root))

if __name__=='__main__':unittest.main()


class PartialProfileRebuild(unittest.TestCase):
    """Changing one profile section only rebuilds the packages that show it."""
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.cooked=Path(self.tmp.name)
        for n in ('GFX_A.upk','GFX_B.upk'):(self.cooked/n).write_bytes(n.encode())
        title={'text':'HOLA','target':'XP'}
        self.profile={'codec_revision':profile.CODEC_REVISION,'settings':{'name':'damo','title':title,'rank':None,'avatar':'','banner':''},
                      'packages':{'GFX_A.upk':{'installed_sha256':profile.sha(self.cooked/'GFX_A.upk'),'features':['name']},
                                  'GFX_B.upk':{'installed_sha256':profile.sha(self.cooked/'GFX_B.upk'),'features':['name','title']}}}

    def tearDown(self):self.tmp.cleanup()

    def settings(self,**change):return {**self.profile['settings'],**change}

    def test_title_change_keeps_name_only_packages(self):
        self.assertEqual(profile._unaffected(self.cooked,self.settings(title={'text':'ADIOS','target':'XP'}),self.profile),{'GFX_A.upk'})

    def test_first_rank_or_old_profile_forces_full_rebuild(self):
        self.assertIsNone(profile._unaffected(self.cooked,self.settings(rank={'tier':1}),self.profile))
        del self.profile['packages']['GFX_A.upk']['features']
        self.assertIsNone(profile._unaffected(self.cooked,self.settings(title={'text':'X','target':'XP'}),self.profile))

    def test_first_banner_keeps_every_menu_package(self):
        self.assertEqual(profile._unaffected(self.cooked,self.settings(banner='x.png'),self.profile),{'GFX_A.upk','GFX_B.upk'})

    def test_package_reset_by_launcher_is_rebuilt(self):
        (self.cooked/'GFX_A.upk').write_bytes(b'epic-original')
        self.assertEqual(profile._unaffected(self.cooked,self.settings(title={'text':'X','target':'XP'}),self.profile),set())
