# Swap Prism

Aplicación de personalización local para Rocket League en Windows: skins, texturas,
sonidos, perfil (nombre, avatar, títulos), fake rank, mapas de Workshop y overlay de FPS.
Todos los cambios son locales, reversibles y se aplican solo con el juego cerrado.

## Descargar

El instalador está en [Releases](https://github.com/damobeixon12/Swap-Prism/releases).
La app avisa cuando hay una versión estable nueva y se actualiza sola, conservando
tus datos y copias de seguridad.

Cierra Rocket League antes de aplicar o restaurar una personalización.

## Comunidad

[Discord de Swap Prism](https://discord.gg/dXa24Y5HtX).

---

## Desarrollo

### Estructura

| Carpeta | Contenido |
|---|---|
| `app/` | Interfaz nativa en Rust (eframe/egui), actualizador `SWAPPRISMUpdate` e instalador NSIS (`installer.nsi`). |
| `app/assets/` | Logo, iconos, fuentes, insignias de rango, catálogo y `THIRD_PARTY.txt`. |
| `engine/` | Motor en Python: edición de paquetes `.upk`, bancos de sonido, perfil, cola de cambios y copias de seguridad. |
| `engine/motor/` | Códec de paquetes (`rl_upk_editor.py`, de RLUPKTools) y base de datos de objetos. |
| `engine/tests/` | Pruebas del motor. |
| `scripts/` | Compilación, empaquetado y publicación. |
| `docs/` | `PENDIENTES.md` y el historial técnico de cada versión (`docs/historial/`). |
| `work/` | Cachés locales: toolchain de Rust, dependencias de compilación, miniaturas. **No se sube.** |
| `outputs/` | Instalador generado y copia de desarrollo `outputs/LocalSkins` con tus datos. **No se sube.** |

La app Rust lanza el motor empaquetado (`_engine/SWAPPRISMEngine.exe`) y le pasa la
carpeta de la app en `SWAPPRISM_ROOT`; los datos del usuario viven en `data/` junto al ejecutable.

### Compilar

Requisitos en Windows: la toolchain de Rust en `work/toolchain` (o `rustup` con
`stable-x86_64-pc-windows-gnu`), Python 3.12 con Pillow y cryptography, y NSIS en
`work/toolchain/nsis-3.11`.

```powershell
# Pruebas del motor
python -m unittest discover -s engine/tests -t engine

# Instalador completo (pruebas + Rust + motor + NSIS) -> outputs/Swap Prism-Setup.exe
powershell -ExecutionPolicy Bypass -File scripts/package_swapprism.ps1

# Comprobar el instalador generado en una carpeta temporal
powershell -ExecutionPolicy Bypass -File scripts/test_profile_installer.ps1
```

Si Python no se encuentra automáticamente, define `SWAPPRISM_PYTHON` con la ruta de `python.exe`.

### Publicar una versión

1. Sube la versión en `app/Cargo.toml`.
2. Ejecuta `Preparar update.cmd`.
3. Sigue `COMO PUBLICAR ACTUALIZACIONES.txt`.

### Reglas importantes

- Nunca escribir paquetes del juego con Rocket League abierto.
- No subir datos personales: `outputs/`, `data/`, copias del juego (`.upk`, `.bnk`) ni sonidos.
- Cada cambio de un paquete guarda el original y debe poder restaurarse byte a byte.
