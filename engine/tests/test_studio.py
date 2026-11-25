import unittest,io,wave,struct,hashlib
from studio_backend import patch_bank,bank_chunks,rgb,with_volume,clip_wave,wem_pcm
from swap_manager import filename
from body_visual import physics_field

def chunk(name,data):return name+struct.pack('<I',len(data))+data
class AudioScope(unittest.TestCase):
    def test_disabled_native_route_cannot_be_applied(self):
        raw,spec=self.fixture();spec['disabled_reason']='Sonido del poste en revisión.'
        with self.assertRaisesRegex(ValueError,'revisión'):patch_bank(raw,spec,b'pcm')
    def test_native_pcm_keeps_samples_with_explicit_layout_and_alignment(self):
        stream=io.BytesIO();samples=struct.pack('<hhh',30000,-20000,123)
        with wave.open(stream,'wb') as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(48000);w.writeframes(samples)
        result=wem_pcm(stream.getvalue())
        self.assertEqual(result[80:],samples)
        self.assertEqual(struct.unpack_from('<I',result,16)[0],40)
        self.assertEqual(struct.unpack_from('<H',result,20)[0],0xfffe)
    def fixture(self):
        hirc=struct.pack('<III',262145,12,0)
        raw=chunk(b'BKHD',b'header')+chunk(b'DIDX',struct.pack('<III',12,0,4)+struct.pack('<III',99,4,5))+chunk(b'DATA',b'old!other')+chunk(b'HIRC',hirc)+chunk(b'STID',b'identity')
        spec={'original_sha256':hashlib.sha256(raw).hexdigest(),'media':[{'id':12,'params':{'StreamType':[{'offset':8,'value':0}],'ulPluginID':[{'offset':0,'value':262145}],'uInMemoryMediaSize':[{'offset':4,'value':12}]}}]}
        return raw,spec
    def test_only_selected_media_and_codec_metadata_change(self):
        raw,spec=self.fixture();result=patch_bank(raw,spec,b'new PCM data!');parts=dict(bank_chunks(result))
        entries=list(struct.iter_unpack('<III',parts[b'DIDX']));other=entries[1]
        self.assertEqual(parts[b'DATA'][other[1]:other[1]+other[2]],b'other')
        self.assertEqual(parts[b'STID'],b'identity');self.assertEqual(parts[b'BKHD'],b'header')
        self.assertEqual(struct.unpack('<III',parts[b'HIRC']),(0x10001,13,0))
    def test_updated_or_truncated_bank_is_rejected(self):
        raw,spec=self.fixture()
        with self.assertRaises(ValueError):patch_bank(raw+b'corruption',spec,b'x')
        with self.assertRaises(ValueError):bank_chunks(raw[:-1])
    def test_streamed_event_is_not_replaced_as_embedded(self):
        raw,spec=self.fixture();spec['media'][0]['params']['StreamType'][0]['value']=1
        with self.assertRaises(ValueError):patch_bank(raw,spec,b'x')
    def test_only_two_identified_audio_banks_are_accepted(self):
        self.assertEqual(filename('SFX_Impacts.bnk'),'SFX_Impacts.bnk')
        for name in ['other.bnk','../SFX_Impacts.bnk','X:/SFX_Impacts.bnk']:
            with self.assertRaises(ValueError):filename(name)
    def test_physics_offsets_do_not_survive_from_donor(self):
        for key in ['HandlingPresetZOffset','FrontAxle','BackAxle','PhysicsAsset','SuspensionTravelMax','ChassisSpring']:self.assertTrue(physics_field(key))
        for key in ['Mesh','AnimTreeTemplate','SkinParameters']:self.assertFalse(physics_field(key))
    def test_bad_colors_are_rejected(self):
        for value in ['red','#GG0000','#123',None]:
            with self.assertRaises(ValueError):rgb(value)
    def test_preview_gain_preserves_duration_and_does_not_amplify(self):
        stream=io.BytesIO()
        with wave.open(stream,'wb') as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(48000);w.writeframes(struct.pack('<hhh',30000,-30000,0))
        with wave.open(io.BytesIO(with_volume(stream.getvalue(),.5)),'rb') as w:
            self.assertEqual(w.getnframes(),3);self.assertEqual(struct.unpack('<hhh',w.readframes(3)),(15000,-15000,0))
        for volume in [2,-1,float('nan')]:
            with self.assertRaises(ValueError):with_volume(stream.getvalue(),volume)
    def test_crop_enforces_selected_interval_instead_of_source_duration(self):
        stream=io.BytesIO()
        with wave.open(stream,'wb') as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(48000);w.writeframes(b'\0\0'*48000*4)
        with wave.open(io.BytesIO(clip_wave(stream.getvalue(),1.,1.5,.6160625)),'rb') as w:self.assertEqual(w.getnframes(),24000)
        for a,b in [(0,2),(-1,0),(2,1),(4,5)]:
            with self.assertRaises(ValueError):clip_wave(stream.getvalue(),a,b,.6160625)

if __name__=='__main__':unittest.main()
