"""Comprueba dentro del motor empaquetado que se puede crear un contexto HTTPS verificado.

Se ejecuta con: SWAPPRISMEngine.exe --run-worker https_context_smoke.py <log.json>
El resultado JSON se escribe en la salida estándar, que el motor redirige al log.
"""
import json
import ssl
import sys

context = ssl.create_default_context()
assert context.check_hostname and context.verify_mode == ssl.CERT_REQUIRED
print(json.dumps({'frozen': getattr(sys, 'frozen', False), 'verified_https_context': True}))
