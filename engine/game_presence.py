"""Detección de procesos de Windows (¿está Rocket League abierto?). Sin acceso a la memoria del juego."""
import ctypes
from ctypes import wintypes as wt
import time


class ProcessEntry(ctypes.Structure):
    _fields_ = [('dwSize', wt.DWORD), ('cntUsage', wt.DWORD), ('th32ProcessID', wt.DWORD),
                ('th32DefaultHeapID', ctypes.c_size_t), ('th32ModuleID', wt.DWORD),
                ('cntThreads', wt.DWORD), ('th32ParentProcessID', wt.DWORD),
                ('pcPriClassBase', wt.LONG), ('dwFlags', wt.DWORD), ('szExeFile', wt.WCHAR * 260)]


class Point(ctypes.Structure):
    _fields_ = [('x', wt.LONG), ('y', wt.LONG)]


class Message(ctypes.Structure):
    _fields_ = [('hwnd', wt.HWND), ('message', wt.UINT), ('wParam', wt.WPARAM),
                ('lParam', wt.LPARAM), ('time', wt.DWORD), ('pt', Point), ('lPrivate', wt.DWORD)]


def windows_api():
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    user = ctypes.WinDLL('user32', use_last_error=True)
    kernel.CreateToolhelp32Snapshot.argtypes = [wt.DWORD, wt.DWORD]
    kernel.CreateToolhelp32Snapshot.restype = wt.HANDLE
    kernel.Process32FirstW.argtypes = [wt.HANDLE, ctypes.POINTER(ProcessEntry)]
    kernel.Process32NextW.argtypes = [wt.HANDLE, ctypes.POINTER(ProcessEntry)]
    kernel.CloseHandle.argtypes = [wt.HANDLE]
    user.GetForegroundWindow.restype = wt.HWND
    user.GetWindowThreadProcessId.argtypes = [wt.HWND, ctypes.POINTER(wt.DWORD)]
    user.PeekMessageW.argtypes = [ctypes.POINTER(Message), wt.HWND, wt.UINT, wt.UINT, wt.UINT]
    user.RegisterHotKey.argtypes = [wt.HWND, ctypes.c_int, wt.UINT, wt.UINT]
    user.UnregisterHotKey.argtypes = [wt.HWND, ctypes.c_int]
    user.SetForegroundWindow.argtypes = [wt.HWND]
    user.IsWindowVisible.argtypes = [wt.HWND]
    user.ShowWindow.argtypes = [wt.HWND, ctypes.c_int]
    return kernel, user


def process_ids(executable='RocketLeague.exe'):
    kernel, _ = windows_api()
    handle = kernel.CreateToolhelp32Snapshot(2, 0)
    if handle == ctypes.c_void_p(-1).value:
        raise OSError(ctypes.get_last_error(), 'No se puede consultar la lista de procesos.')
    found = set()
    try:
        entry = ProcessEntry()
        entry.dwSize = ctypes.sizeof(entry)
        success = kernel.Process32FirstW(handle, ctypes.byref(entry))
        while success:
            if entry.szExeFile.casefold() == executable.casefold():
                found.add(entry.th32ProcessID)
            success = kernel.Process32NextW(handle, ctypes.byref(entry))
    finally:
        kernel.CloseHandle(handle)
    return found

