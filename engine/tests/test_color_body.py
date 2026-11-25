import struct,unittest
from types import SimpleNamespace
from cosmetic_colors import color_ranges,disable_color_overrides,particle_color_ranges,override_color_ranges
from body_visual import validate_visual_change

NAMES=['None','StartColor','StructProperty','RawDistributionVector','Distribution','ObjectProperty','LookupTable',
       'ArrayProperty','Constant','Vector','ParameterName','NameProperty','CustomColor','Mesh','HandlingPreset','FrontAxle',
       'ParameterOverrides','PaintParameterName','Paint','LinearColor']
def fname(name):return struct.pack('<ii',NAMES.index(name),0)
def prop(name,kind,value,struct_name=None):
    return fname(name)+fname(kind)+struct.pack('<ii',len(value),0)+(fname(struct_name) if struct_name else b'')+value
def stream(*props):return b''.join(props)+fname('None')
def fake_package(objects):
    flat=bytearray(5000);exports=[]
    for ix,(cls,data) in enumerate(objects):
        exp=bytearray(72);struct.pack_into('<i',exp,32,len(data));struct.pack_into('<q',exp,36,100+ix*1000)
        flat[100+ix*1000:100+ix*1000+len(data)]=data;exports.append(exp)
    return SimpleNamespace(names=NAMES,exports=exports,flat=lambda:bytes(flat),
        class_name=lambda exp:objects[next(i for i,e in enumerate(exports) if e is exp)][0],path=lambda i:'object'+str(i))

class ParticleColorScope(unittest.TestCase):
    def test_runtime_paint_override_color_and_its_palette_link_are_found(self):
        record=stream(prop('PaintParameterName','NameProperty',fname('CustomColor')),
                      prop('Paint','ObjectProperty',struct.pack('<i',-5)),
                      prop('CustomColor','StructProperty',struct.pack('<ffff',1,.5,.2,.8),'LinearColor'))
        data=struct.pack('<i',0)+stream(prop('ParameterOverrides','ArrayProperty',struct.pack('<i',1)+record))
        pkg=fake_package([('ProductOverride_ParticleSystemColorParameter_TA',data)])
        ranges,controls=override_color_ranges(pkg,pkg.flat())
        self.assertEqual(len(ranges),1);self.assertEqual(len(controls),1)
        self.assertTrue(next(iter(ranges.values()))['runtime_override'])

    def setUp(self):
        table=struct.pack('<i8f',8,0,1,1,.2,.1,1,.2,.1)
        raw=stream(prop('Distribution','ObjectProperty',struct.pack('<i',2)),prop('LookupTable','ArrayProperty',table))
        module=struct.pack('<i',0)+stream(prop('StartColor','StructProperty',raw,'RawDistributionVector'))
        color=struct.pack('<ii',0,0)+stream(prop('Constant','StructProperty',struct.pack('<fff',1,.2,.1),'Vector'),prop('ParameterName','NameProperty',fname('CustomColor')))
        spatial=struct.pack('<ii',0,0)+stream(prop('Constant','StructProperty',struct.pack('<fff',10,20,30),'Vector'),prop('ParameterName','NameProperty',fname('CustomColor')))
        self.pkg=fake_package([('ParticleModuleColor',module),('DistributionVectorParticleParameter',color),('DistributionVectorParticleParameter',spatial)])
    def test_color_module_lookup_and_its_distribution_are_both_edited(self):
        ranges=color_ranges(self.pkg)
        self.assertEqual(len(ranges),3)
        self.assertFalse(any(2100<=p<3100 for p in ranges))
    def test_only_color_dependency_loses_runtime_parameter_override(self):
        before=self.pkg.flat();result=bytearray(before);_,deps=particle_color_ranges(self.pkg,before)
        changed,allowed=disable_color_overrides(self.pkg,before,result,deps)
        self.assertEqual(len(changed),1);self.assertTrue(allowed)
        self.assertEqual(before[2100:3100],result[2100:3100])

class BodyPhysicsInvariant(unittest.TestCase):
    def body(self,mesh=1,handling=-1):
        return struct.pack('<i',0)+stream(prop('Mesh','ObjectProperty',struct.pack('<i',mesh)),
            prop('HandlingPreset','ObjectProperty',struct.pack('<i',handling)),prop('FrontAxle','StructProperty',struct.pack('<fff',1,2,3),'Vector'))
    def test_mesh_change_preserves_all_original_physics_data(self):
        old=fake_package([('ProductAsset_Body_TA',self.body()),('PhysicsAsset',b'original-physics')])
        new=fake_package([('ProductAsset_Body_TA',self.body(mesh=-5)),('PhysicsAsset',b'original-physics')])
        self.assertTrue(validate_visual_change(old,new,0,{'Mesh'})['original_body_physics_data_unchanged'])
    def test_handling_or_collision_changes_are_rejected(self):
        old=fake_package([('ProductAsset_Body_TA',self.body()),('PhysicsAsset',b'original-physics')])
        for objects in ([('ProductAsset_Body_TA',self.body(mesh=-5,handling=-2)),('PhysicsAsset',b'original-physics')],
                        [('ProductAsset_Body_TA',self.body(mesh=-5)),('PhysicsAsset',b'changed--physics')]):
            with self.assertRaises(ValueError):validate_visual_change(old,fake_package(objects),0,{'Mesh'})
        with self.assertRaises(ValueError):validate_visual_change(old,old,0,{'Mesh','HandlingPreset'})

if __name__=='__main__':unittest.main()
