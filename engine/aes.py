"""AES-ECB / AES-CTR through Windows' own CNG (bcrypt.dll), so the engine needs no crypto package."""
import ctypes
from ctypes import wintypes

_bcrypt=ctypes.WinDLL('bcrypt')
_alg=None


def _check(status,what):
    if status:raise OSError(f'{what} falló (0x{status & 0xFFFFFFFF:08X}).')


def _provider():
    global _alg
    if _alg is None:
        handle=ctypes.c_void_p()
        _check(_bcrypt.BCryptOpenAlgorithmProvider(ctypes.byref(handle),'AES',None,0),'BCryptOpenAlgorithmProvider')
        mode=ctypes.create_unicode_buffer('ChainingModeECB')
        _check(_bcrypt.BCryptSetProperty(handle,'ChainingMode',mode,ctypes.sizeof(mode),0),'BCryptSetProperty')
        _alg=handle
    return _alg


def _ecb(key,data,encrypt):
    if len(data)%16:raise ValueError('Los datos AES deben ser múltiplo de 16 bytes.')
    if not data:return b''
    handle=ctypes.c_void_p();secret=ctypes.create_string_buffer(bytes(key),len(key))
    _check(_bcrypt.BCryptGenerateSymmetricKey(_provider(),ctypes.byref(handle),None,0,secret,len(key),0),'BCryptGenerateSymmetricKey')
    try:
        source=ctypes.create_string_buffer(bytes(data),len(data));out=ctypes.create_string_buffer(len(data));done=wintypes.ULONG()
        call=_bcrypt.BCryptEncrypt if encrypt else _bcrypt.BCryptDecrypt
        _check(call(handle,source,len(data),None,None,0,out,len(data),ctypes.byref(done),0),'BCryptEncrypt' if encrypt else 'BCryptDecrypt')
        return out.raw[:done.value]
    finally:_bcrypt.BCryptDestroyKey(handle)


def ecb_encrypt(key,data):return _ecb(key,data,True)
def ecb_decrypt(key,data):return _ecb(key,data,False)


def ctr(key,counter,data):
    """AES-CTR with a 16-byte initial counter block, incremented as one big-endian 128-bit number."""
    if not data:return b''
    start=int.from_bytes(counter,'big');blocks=(len(data)+15)//16
    stream=ecb_encrypt(key,b''.join(((start+i)&((1<<128)-1)).to_bytes(16,'big') for i in range(blocks)))[:len(data)]
    return (int.from_bytes(data,'big')^int.from_bytes(stream,'big')).to_bytes(len(data),'big')
