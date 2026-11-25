import json,tempfile,unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
import swap_manager as swaps
import change_queue as batch
from menu_guard import EditState

class BatchTransaction(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.root=Path(self.tmp.name)
        self.game=self.root/'game';self.cooked=self.game/'TAGame/CookedPCConsole';self.cooked.mkdir(parents=True)
        self.api=SimpleNamespace(ROOT=self.root,safe_package=swaps.filename);self.jobs=[]
        for name in ('one.upk','two.upk'):
            (self.cooked/name).write_bytes(b'original-'+name.encode())
            job=self.root/name.removesuffix('.upk');(job/'generado').mkdir(parents=True)
            (job/'generado'/name).write_bytes(b'replacement-'+name.encode())
            report={'target':{'ID':1,'Product':name,'AssetPackage':name},'donor':{'ID':2,'Product':'New'},
                    'validation':{'serialized_payloads_unchanged':True},'original_sha256':{name:swaps.sha(self.cooked/name)},
                    'generated_sha256':swaps.sha(job/'generado'/name)}
            (job/'informe.json').write_text(json.dumps(report));self.jobs.append(job)
        self.gates=[patch('change_queue.require_closed',return_value=EditState('closed','test')),
                    patch('swap_manager.require_editable',return_value=EditState('closed','test'))]
        for gate in self.gates:gate.start()
    def tearDown(self):
        for gate in self.gates:gate.stop()
        self.tmp.cleanup()
    def test_full_batch_installs_with_individual_original_backups(self):
        self.assertEqual(batch.install_all(self.api,self.jobs,self.game,lambda s:None),2)
        self.assertEqual(len(swaps.load_state(self.root/'data')),2)
        self.assertEqual(swaps.restore_all(self.root/'data'),2)

    def test_last_boost_wins_but_other_categories_remain(self):
        first={'kind':'item','target':{'Slot':'Boost'},'donor':{'ID':1}}
        second={'kind':'item','target':{'Slot':'Boost'},'donor':{'ID':2}}
        wheel={'kind':'item','target':{'Slot':'Wheels'}}
        self.assertEqual(batch.latest_per_category([first,wheel,second]),[second,wheel])

    def test_replacing_original_source_restores_previous_boost_in_transaction(self):
        job=self.jobs[0];report=json.loads((job/'informe.json').read_text());report['target']['Slot']='Boost'
        (job/'informe.json').write_text(json.dumps(report))
        batch.install_all(self.api,[job],self.game,lambda s:None)
        plans=[{'kind':'item','target':{'Slot':'Boost'}}]
        extras=batch.superseded_jobs(self.api,plans,[self.jobs[1]],self.game)
        self.assertEqual(len(extras),1)
        batch.install_all(self.api,extras+[self.jobs[1]],self.game,lambda s:None)
        self.assertEqual((self.cooked/'one.upk').read_bytes(),b'original-one.upk')
        self.assertEqual(swaps.load_state(self.root/'data')['one.upk']['status'],'restored')
    def test_later_failure_rolls_back_earlier_success(self):
        real=batch.apply_job;calls=[]
        def second_fails(*args):
            calls.append(args)
            if len(calls)==2:raise ValueError('Simulated write failure')
            return real(*args)
        with patch('change_queue.apply_job',side_effect=second_fails),self.assertRaises(ValueError):
            batch.install_all(self.api,self.jobs,self.game,lambda s:None)
        for name in ('one.upk','two.upk'):self.assertEqual((self.cooked/name).read_bytes(),b'original-'+name.encode())
        self.assertEqual(swaps.load_state(self.root/'data'),{})
    def test_running_game_blocks_batch_before_mutation(self):
        with patch('change_queue.require_closed',side_effect=ValueError('Close game')),self.assertRaises(ValueError):
            batch.install_all(self.api,self.jobs,self.game,lambda s:None)
        self.assertFalse((self.root/'data').exists())

    def test_rollback_does_not_overwrite_external_update(self):
        real=batch.apply_job;calls=[]
        def update_during_second(*args):
            calls.append(args)
            if len(calls)==2:
                (self.cooked/'two.upk').write_bytes(b'epic-update')
                raise ValueError('External update')
            return real(*args)
        with patch('change_queue.apply_job',side_effect=update_during_second),self.assertRaises(ValueError):
            batch.install_all(self.api,self.jobs,self.game,lambda s:None)
        self.assertEqual((self.cooked/'two.upk').read_bytes(),b'epic-update')
        self.assertEqual((self.cooked/'one.upk').read_bytes(),b'original-one.upk')

if __name__=='__main__':unittest.main()
