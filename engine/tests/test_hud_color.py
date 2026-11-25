import unittest
from hud_abc import neutralize_color_method,encode_u30,decode_color_instructions

class HudColorOnly(unittest.TestCase):
    def setUp(self):
        names={1:'COLOR_NORMAL',2:'COLOR_GLOW_RED',3:'COLOR_GLOW_AMBER',4:'COLOR_BOOST_MAX',5:'COLOR_BOOST_NORMAL',
               6:'COLOR_BACKGROUND_AMBER',7:'tagame.hud:BoostMeterView.m_teamStyle',8:'colorTransform',9:'m_displayBoost'}
        self.name=names.__getitem__
        self.counter=b'\xd0\x66'+encode_u30(9)+b'\x29'
        self.code=b'\xd0\x30'+self.counter+b''.join(b'\x60'+encode_u30(i)+b'\x29' for i in range(1,7))
        self.code+=b'\xd0\x66'+encode_u30(7)+b'\x66'+encode_u30(8)+b'\x29\x47'
    def test_display_colors_change_without_changing_counter_access_or_offsets(self):
        result,edits=neutralize_color_method(self.code,self.name)
        self.assertEqual(len(result),len(self.code));self.assertEqual(len(edits),6)
        self.assertEqual(result[2:2+len(self.counter)],self.counter)
        allowed={i for a,b in edits for i in range(a,b)}
        self.assertTrue(all(a==b or i in allowed for i,(a,b) in enumerate(zip(self.code,result))))
        decode_color_instructions(result)
    def test_unknown_hud_profile_is_rejected(self):
        with self.assertRaises(ValueError):neutralize_color_method(self.code[:-7]+b'\x47',self.name)
        with self.assertRaises(ValueError):neutralize_color_method(b'\xff'+self.code,self.name)
    def test_color_update_method_can_reuse_the_verified_neutral_constant(self):
        code=self.counter+b'\x60\x02\x29\x47'
        result,edits=neutralize_color_method(code,self.name,identity_id=1,require_full_profile=False)
        self.assertEqual(result[:len(self.counter)],self.counter)
        self.assertEqual(len(result),len(code));self.assertEqual(len(edits),1)

if __name__=='__main__':unittest.main()
