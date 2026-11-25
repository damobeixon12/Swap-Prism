"""Comprobación de solo lectura dentro del motor empaquetado, incluido su códec de paquetes.

Abre Startup.upk del juego instalado sin modificarlo y confirma que no se carga Tkinter.
Se ejecuta con: SWAPPRISMEngine.exe --run-worker profile_codec_smoke.py <log.json>
"""
import json
import sys

import local_skins as api
from modern_upk import Package
from swap_manager import sha

game, _ = api.detect_epic()
source = game / 'TAGame/CookedPCConsole/Startup.upk'
before = sha(source)
package = Package(source, api.DEFAULT_ENGINE)
assert 'tkinter' not in sys.modules
assert any(package.class_name(e) == 'Texture2D' and package.path(i) == 'gfx_shared.Icon_AvatarDefault'
           for i, e in enumerate(package.exports))
assert sha(source) == before
print(json.dumps({'frozen': getattr(sys, 'frozen', False), 'codec_loaded': True, 'tkinter_loaded': False,
                  'game_writes': False, 'exports': len(package.exports)}))
