"""Fail-closed gate: cosmetic mutations only when closed or confirmed main menu."""
from dataclasses import dataclass
from pathlib import Path
import ctypes
from ctypes import wintypes as wt
import re
from game_presence import process_ids
from player_inventory import save_folders


@dataclass(frozen=True)
class EditState:
    mode:str
    reason:str

    @property
    def allowed(self):
        return self.mode in ('closed','menu')


def parse_world(text):
    transitions=list(re.finditer(r'(?:DevNet:\s*Browse:|Log:\s*LoadMap:)\s*([^\s?]+)',text))
    if not transitions:
        return EditState('unknown','Estado del juego desconocido: edición bloqueada.')
    last=transitions[-1]
    if 'Browse:' in last.group(0):
        return EditState('loading','El juego está cambiando de escenario: edición bloqueada.')
    world=last.group(1).casefold()
    if world in ('menu_main_p','menu_main'):
        return EditState('menu','Menú principal confirmado.')
    return EditState('match','Partida, entrenamiento o escenario activo: edición bloqueada.')


def process_started(pid):
    kernel=ctypes.WinDLL('kernel32',use_last_error=True)
    kernel.OpenProcess.argtypes=[wt.DWORD,wt.BOOL,wt.DWORD]
    kernel.OpenProcess.restype=wt.HANDLE
    kernel.GetProcessTimes.argtypes=[wt.HANDLE,*([ctypes.POINTER(wt.FILETIME)]*4)]
    kernel.CloseHandle.argtypes=[wt.HANDLE]
    handle=kernel.OpenProcess(0x1000,False,pid)
    if not handle:
        return None
    try:
        times=[wt.FILETIME() for _ in range(4)]
        if not kernel.GetProcessTimes(handle,*(ctypes.byref(value) for value in times)):
            return None
        stamp=(times[0].dwHighDateTime<<32)|times[0].dwLowDateTime
        return (stamp-116444736000000000)/10000000
    finally:
        kernel.CloseHandle(handle)


def edit_state():
    try:
        pids=process_ids()
        if not pids:
            return EditState('closed','Rocket League cerrado.')
        if len(pids)!=1:
            return EditState('unknown','Hay varias instancias del juego: edición bloqueada.')
        started=process_started(next(iter(pids)))
        logs=[folder.parent/'Logs'/'Launch.log' for folder in save_folders()]
        logs=[path for path in logs if path.is_file()]
        if not logs or started is None:
            return EditState('unknown','No se puede confirmar la sesión actual: edición bloqueada.')
        log=max(logs,key=lambda path:path.stat().st_mtime)
        if log.stat().st_mtime < started-2:
            return EditState('unknown','El registro corresponde a otra sesión: edición bloqueada.')
        with log.open('rb') as stream:
            stream.seek(0,2)
            length=stream.tell()
            stream.seek(max(0,length-2*1024*1024))
            text=stream.read().decode('utf-8',errors='replace')
        return parse_world(text)
    except (OSError,ValueError):
        return EditState('unknown','No se puede leer el estado actual: edición bloqueada.')


def require_editable():
    state=edit_state()
    if not state.allowed:
        raise ValueError(state.reason)
    return state


def require_closed():
    if process_ids():
        raise ValueError('Cierra Rocket League para aplicar o restaurar cambios.')
    return EditState('closed','Rocket League cerrado.')
