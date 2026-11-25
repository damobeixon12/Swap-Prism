"""Embed supplied ICO in a newly built local executable, using Windows resources."""
import ctypes,struct,sys
from pathlib import Path
def embed(exe,ico):
    data=Path(ico).read_bytes();reserved,kind,count=struct.unpack_from('<HHH',data)
    if reserved or kind!=1:raise ValueError('ICO inválido')
    kernel=ctypes.WinDLL('kernel32',use_last_error=True)
    kernel.BeginUpdateResourceW.argtypes=[ctypes.c_wchar_p,ctypes.c_int];kernel.BeginUpdateResourceW.restype=ctypes.c_void_p
    kernel.UpdateResourceW.argtypes=[ctypes.c_void_p,ctypes.c_void_p,ctypes.c_void_p,ctypes.c_ushort,ctypes.c_void_p,ctypes.c_uint]
    kernel.EndUpdateResourceW.argtypes=[ctypes.c_void_p,ctypes.c_int]
    handle=kernel.BeginUpdateResourceW(str(Path(exe).resolve()),False)
    if not handle:raise ctypes.WinError(ctypes.get_last_error())
    try:
        group=bytearray(struct.pack('<HHH',0,1,count))
        for index in range(count):
            entry=data[6+index*16:22+index*16];size,offset=struct.unpack_from('<II',entry,8);image=data[offset:offset+size];blob=ctypes.create_string_buffer(image)
            if not kernel.UpdateResourceW(handle,3,index+1,0,blob,len(image)):raise ctypes.WinError(ctypes.get_last_error())
            group.extend(entry[:12]+struct.pack('<H',index+1))
        blob=ctypes.create_string_buffer(bytes(group))
        if not kernel.UpdateResourceW(handle,14,1,0,blob,len(group)):raise ctypes.WinError(ctypes.get_last_error())
    except BaseException:kernel.EndUpdateResourceW(handle,True);raise
    if not kernel.EndUpdateResourceW(handle,False):raise ctypes.WinError(ctypes.get_last_error())
if __name__=='__main__':embed(*sys.argv[1:])
