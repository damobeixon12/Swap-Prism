import tempfile,unittest,json
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
import swapprism_engine as bridge

class NativeBridge(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.root=Path(self.tmp.name)
        (self.root/'TAGame/CookedPCConsole').mkdir(parents=True);(self.root/'Binaries/Win64').mkdir(parents=True)
        (self.root/'Binaries/Win64/RocketLeague.exe').write_bytes(b'test')
        self.target={'ID':1,'Product':'Owned wheel','Slot':'Wheels','AssetPackage':'wheel.upk'}
        self.donor={'ID':2,'Product':'Donor wheel','Slot':'Wheels','AssetPackage':'donor.upk'}
        self.snapshot=SimpleNamespace(inventory=[self.target],active={'teams':[{'items':[{'Slot':'Body','Product':'Fennec'}]},{'items':[{'Slot':'Body','Product':'Dominus'}]}]})
        self.request={'action':'apply','game':str(self.root),'team':'Orange','plans':[{'kind':'item','target_id':1,'donor_id':2,'label':'Wheel swap'}]}
        self.patches=[patch.object(bridge,'require_closed'),patch.object(bridge.api,'ROOT',self.root),patch.object(bridge.api,'read_database',return_value=[self.target,self.donor]),patch.object(bridge,'load_snapshot',return_value=self.snapshot),patch.object(bridge,'load_library',return_value=[]),patch.object(bridge,'load_state',return_value={})]
        for p in self.patches:p.start()
    def tearDown(self):
        for p in reversed(self.patches):p.stop()
        self.tmp.cleanup()
    def test_orange_team_car_is_taken_from_real_preset(self):
        with patch.object(bridge,'prepare_all',return_value=['job']) as prepare,patch.object(bridge,'install_all',return_value=1):
            result=bridge.request(self.request)
            self.assertEqual(result['count'],1);self.assertEqual(prepare.call_args.args[1][0]['car'],'Dominus')
    def test_profile_and_cosmetic_same_package_install_once(self):
        cosmetic=self.root/'cosmetic';profile_job=self.root/'profile'
        for job in [cosmetic,profile_job]:
            job.mkdir();(job/'informe.json').write_text(json.dumps({'target':{'AssetPackage':'wheel.upk'}}))
        new={'schema':1,'settings':{'name':'Alias'},'packages':{}}
        self.request['profile_settings']={'name':'Alias'}
        with patch.object(bridge,'prepare_all',return_value=[cosmetic]),patch.object(bridge.profile_manager,'prepare',return_value=([profile_job],new)),patch.object(bridge.profile_manager,'compose_jobs',return_value=new) as compose,patch.object(bridge.profile_manager,'commit_jobs') as commit,patch.object(bridge.profile_manager,'commit_composed'),patch.object(bridge,'install_all',return_value=1) as install:
            bridge.request(self.request)
            compose.assert_called_once_with(bridge.api,[cosmetic],new)
            install.assert_called_once_with(bridge.api,[cosmetic],self.root)
            commit.assert_called_once_with(bridge.api,[],new)
    def test_batch_failure_keeps_profile_metadata_uncommitted(self):
        new={'schema':1,'settings':{'name':'Alias'},'packages':{}}
        self.request['plans']=[];self.request['profile_settings']={'name':'Alias'}
        with patch.object(bridge,'prepare_all',return_value=[]),patch.object(bridge.profile_manager,'prepare',return_value=([],new)),patch.object(bridge.profile_manager,'compose_jobs',return_value=new),patch.object(bridge.profile_manager,'commit_jobs') as commit,patch.object(bridge,'install_all',side_effect=ValueError('write failed')):
            with self.assertRaises(ValueError):bridge.request(self.request)
            commit.assert_not_called()
    def test_unowned_original_is_rejected_before_preparation(self):
        self.snapshot.inventory=[]
        with patch.object(bridge,'prepare_all') as prepare:
            with self.assertRaisesRegex(ValueError,'inventario'):bridge.request(self.request)
            prepare.assert_not_called()
    def test_blueprints_do_not_count_as_owned(self):
        self.snapshot.inventory=[dict(self.target,blueprint=True)]
        with patch.object(bridge,'prepare_all') as prepare:
            with self.assertRaises(ValueError):bridge.request(self.request)
            prepare.assert_not_called()
    def test_bad_game_folder_is_rejected(self):
        self.request['game']=str(self.root/'TAGame')
        with patch.object(bridge,'prepare_all') as prepare:
            with self.assertRaisesRegex(ValueError,'Binaries'):bridge.request(self.request)
            prepare.assert_not_called()
    def test_individual_restore_keeps_selection(self):
        with patch.object(bridge,'restore_all',return_value=1) as restore:
            bridge.request({'action':'restore','names':['wheel.upk']})
            restore.assert_called_once_with(self.root/'data',names=['wheel.upk'])
    def test_meter_restore_includes_only_its_tracked_companions(self):
        state={'GFxShell_SF.upk':{'hud_display_companion':True},'GFx_ReplayHud_SF.upk':{'hud_display_companion':True},
               'GFX_SpectatorHud_SF.upk':{'status':'installed'},'other.upk':{'hud_display_companion':True}}
        with patch.object(bridge,'load_state',return_value=state),patch.object(bridge,'restore_all',return_value=3) as restore:
            bridge.request({'action':'restore','names':['GFX_Hud_SF.upk']})
            restore.assert_called_once_with(self.root/'data',names=['GFX_Hud_SF.upk','GFxShell_SF.upk','GFx_ReplayHud_SF.upk'])
    def test_restore_does_not_run_when_the_game_is_open(self):
        with patch.object(bridge,'require_closed',side_effect=ValueError('Close game')),patch.object(bridge,'restore_all') as restore:
            with self.assertRaises(ValueError):bridge.request({'action':'restore'})
            restore.assert_not_called()

class WelcomeName(unittest.TestCase):
    def test_primary_player_must_match_the_save_account(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)/'TAGame';(root/'Logs').mkdir(parents=True)
            source=root/'SaveDataEpic/DBE_Production/account123_2.save'
            (root/'Logs/Launch.log').write_text(
                'Party: HandleLocalPlayerLoginStatusChanged PlayerName=Wrong PlayerID=Epic|other|0 LoginStatus=LS_LoggedIn IsPrimary=True\n'
                'Party: HandleLocalPlayerLoginStatusChanged PlayerName=Real Player Name PlayerID=Epic|account123|0 LoginStatus=LS_LoggedIn IsPrimary=True\n',encoding='utf8')
            self.assertEqual(bridge.player_name(source),'Real Player Name')
            self.assertEqual(bridge.player_name(source.with_name('different_2.save')),'')
    def test_missing_login_does_not_invent_a_name(self):
        self.assertEqual(bridge.player_name(Path('fake.save')),'')

if __name__=='__main__':unittest.main()
