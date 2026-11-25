import unittest
import aes


class WindowsAes(unittest.TestCase):
    def test_fips197_vector_and_ctr(self):
        key=bytes(range(16));plain=bytes.fromhex('00112233445566778899aabbccddeeff')
        self.assertEqual(aes.ecb_encrypt(key,plain).hex(),'69c4e0d86a7b0430d8cdb78070b4c55a')
        self.assertEqual(aes.ecb_decrypt(key,aes.ecb_encrypt(key,plain)),plain)
        counter=bytes(15)+b'\xff';data=bytes(range(40))
        self.assertEqual(aes.ctr(key,counter,aes.ctr(key,counter,data)),data)
        # Block 2 uses counter+1, carried into the next byte.
        self.assertEqual(aes.ctr(key,counter,bytes(32))[16:],aes.ecb_encrypt(key,bytes(14)+b'\x01\x00'))
