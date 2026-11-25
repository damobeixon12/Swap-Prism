import unittest
import save_reader as sr
from game_settings import bindings

def prop(name,tag,value):
    return sr.write_ue3(name)+sr.write_ue3(tag)+bytes(8)+sr.write_ue3(value)

class Bindings(unittest.TestCase):
    def test_reads_bound_key(self):
        data=b'junk'+prop('Action','NameProperty','ToggleScoreboard')+prop('Key','NameProperty','XboxTypeS_LeftThumbStick')+sr.write_ue3('None')
        data+=prop('Action','NameProperty','Jump')+prop('Key','NameProperty','XboxTypeS_A')
        self.assertEqual(bindings(data,'ToggleScoreboard'),['XboxTypeS_LeftThumbStick'])
    def test_none_bound(self):
        self.assertEqual(bindings(b'nothing','ToggleScoreboard'),[])

if __name__=='__main__':unittest.main()
