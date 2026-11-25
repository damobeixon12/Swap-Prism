import struct,unittest
from post_isolation import objects,validate_layout,pack_aligned
from studio_backend import bank_chunks

class HircBoundaries(unittest.TestCase):
    def chunks(self,ids=(1,2)):
        header=bytearray(40);struct.pack_into('<IIII',header,0,150,123,0,16)
        index=b''.join(struct.pack('<III',ident,16*n,16) for n,ident in enumerate(ids))
        return [(b'BKHD',bytes(header)),(b'DIDX',index),(b'DATA',b'x'*16*len(ids))]
    def test_relative_alignment_is_not_enough_when_index_grows(self):
        raw=b''.join(tag+struct.pack('<I',len(data))+data for tag,data in self.chunks())
        with self.assertRaisesRegex(ValueError,'absoluta'):validate_layout(raw,bank_chunks)
    def test_header_padding_aligns_absolute_samples_without_changing_audio(self):
        chunks=self.chunks();raw=pack_aligned(chunks,{},bank_chunks);proof=validate_layout(raw,bank_chunks)
        self.assertEqual(proof['data_start']%16,0)
        self.assertEqual(dict(bank_chunks(raw))[b'DATA'],dict(chunks)[b'DATA'])
        self.assertEqual(dict(bank_chunks(raw))[b'BKHD'][:40],dict(chunks)[b'BKHD'])
    def test_unordered_and_duplicate_media_indices_are_rejected(self):
        for ids in [(2,1),(1,1)]:
            with self.assertRaisesRegex(ValueError,'ordenado'):pack_aligned(self.chunks(ids),{},bank_chunks)
    def test_rejects_duplicate_objects_and_trailing_data(self):
        obj=b'\2'+struct.pack('<II',4,123)
        self.assertEqual(objects(struct.pack('<I',1)+obj)[0][0],123)
        for raw in [struct.pack('<I',2)+obj+obj,struct.pack('<I',1)+obj+b'extra',struct.pack('<I',1)+obj[:-1],b'']:
            with self.assertRaises(ValueError):objects(raw)

if __name__=='__main__':unittest.main()
