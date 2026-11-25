# Swap Prism — contexto para Claude Code

App de Windows para personalizar Rocket League **en local**: skins, texturas, sonidos,
perfil (nombre/avatar/títulos), fake rank, mapas de Workshop, overlay de FPS y pestaña de Tracker.
Todo se hace editando los paquetes del juego (`.upk`, `.bnk`) con copia de seguridad y restauración byte a byte.
Versión actual: **0.29.1** (`app/Cargo.toml`). Repo: https://github.com/damobeixon12/Swap-Prism
(hasta ahora solo tenía README y Releases; el código aún NO está en git).

Creada originalmente con Codex. El 25/11/2026 se reorganizó entera (ver «Estado de la reorganización»).

## Arquitectura

```
app/ (Rust, eframe/egui 0.31)  --stdin JSON-->  _engine/SWAPPRISMEngine.exe (Python 3.12, PyInstaller)
     SWAPPRISM.exe                              motor de engine/ ; SWAPPRISM_ROOT = carpeta de la app
     SWAPPRISMUpdate.exe (actualizador nativo)  datos del usuario en <app>/data/
```
- `app/src/main.rs`: arranque, protocolo con el motor (`worker_progress`), modos `--smoke`, `--fps-check`…
- `screens.rs`, `navigation.rs`, `design.rs`, `motion.rs`, `icons.rs`: interfaz (tema oscuro forzado, fuente Nunito Medium).
- `overlay.rs`, `telemetry.rs`, `platform.rs`: overlay FPS con PresentMon (`app/vendor`), ventana transparente por colorkey.
- `update.rs`, `update_runner.rs`, `update_panel.rs`: actualizaciones desde GitHub Releases (`update.json` + SHA-256).
- `tracker.rs`: web oficial de Tracker en WebView2 (Wry). `ranks.rs`: insignias. `workshop.rs`, `media.rs`, `studio.rs`, `titles.rs`, `profile.rs`.
- `engine/swapprism_engine.py`: punto de entrada (`request(obj)` por stdin, o `--run-worker script log`).
- `engine/local_skins.py`: API común (ROOT, DEFAULT_ENGINE, catálogo, detección de Epic).
- `engine/swap_manager.py`, `change_queue.py`, `menu_guard.py`, `game_presence.py`: cola de cambios, transacciones, backups, comprobación de juego cerrado.
- `engine/modern_upk.py` + `engine/motor/rl_upk_editor.py` (RLUPKTools): lectura/escritura de paquetes cifrados.
- Funciones: `cosmetic_backend.py`, `body_visual.py`, `texture_engine.py`, `profile_manager.py`, `fake_display.py`, `title_display.py`, `rank_display.py`, `hud_abc.py`/`avm2_read.py` (ActionScript del HUD), `studio_backend.py` + `post_isolation.py` (sonidos Wwise), `start_logo.py`, `workshop.py`, `media_store.py`.

## Estructura

| Carpeta | Qué hay | ¿En git? |
|---|---|---|
| `app/` | Rust, `installer.nsi`, `assets/`, `vendor/PresentMon.exe` | sí |
| `engine/` | motor Python, `motor/`, `tests/` | sí |
| `scripts/` | compilación y publicación | sí |
| `docs/` | `PENDIENTES.md`, `historial/` (un .md por versión con detalles técnicos) | sí |
| `work/` | toolchain Rust, `builddeps` (PyInstaller, miniaudio), NSIS, cachés | no |
| `outputs/` | instalador generado y copia de desarrollo `outputs/LocalSkins` con **datos personales** | no |

## Compilar y probar (Windows)

```powershell
python -m unittest discover -s engine/tests -t engine          # 86 pruebas
powershell -ExecutionPolicy Bypass -File scripts/package_swapprism.ps1   # pruebas + Rust + motor + NSIS
powershell -ExecutionPolicy Bypass -File scripts/test_profile_installer.ps1  # prueba del instalador
```
- `scripts/build_rust.ps1` usa `work/toolchain` (target `x86_64-pc-windows-gnu`, rust-lld) o cargo del sistema.
- `scripts/python_path.ps1` busca Python 3.12 (SWAPPRISM_PYTHON → runtime de Codex → `py -3.12` → python).
- Publicar: subir versión en `app/Cargo.toml` → `Preparar update.cmd` → seguir `COMO PUBLICAR ACTUALIZACIONES.txt`.

## Reglas obligatorias

- **Nunca escribir/aplicar/restaurar paquetes del juego con Rocket League abierto.** No cerrar partidas.
- No tocar `outputs/LocalSkins/data` ni `%LOCALAPPDATA%\Programs\SWAPPRISM\data` (backups, perfil, selecciones).
- No dar nada por funcionando en el juego sin confirmación del usuario; probar sobre copias no cuenta.
- No inventar IDs/nombres/imágenes de objetos. No enviar datos del guardado/perfil a servicios externos.
- No publicar Releases ni hacer push sin que el usuario lo pida.
- Pedir permiso antes de pasar a otra tarea grande; el usuario escribe en español.

## Estado de la reorganización (25/11/2026)

- Proyecto movido de `Documents\Codex\2026-11-18\ho` a `Documents\SwapPrism`; 256 rutas absolutas de `outputs/LocalSkins/data` reescritas.
- Material de investigación, panel Tkinter antiguo y scripts de un uso archivados en `Documents\SwapPrism-archivo` (`MOVIMIENTOS.txt`).
- Motor sacado de `outputs/LocalSkins` a `engine/`; scripts de `work/` a `scripts/`; 6 funciones muertas eliminadas.
- Las pruebas Python pasan. **Los scripts .ps1 reescritos y la compilación completa aún NO se han ejecutado** tras la reorganización (`app/target` estaba limpio). Lo primero: ejecutar `scripts/package_swapprism.ps1` y corregir lo que falle.
- Después: `git init`, primer commit y push al repo (revisar que `.gitignore` excluye `outputs/` y `work/`).

## Pendiente de la app

Ver `docs/PENDIENTES.md`. Resumen:
- Sin confirmar en juego: fake rank (MMR 1600), color del flip reset, nombre al marcar, cambio de modelo (Body), reinicio del actualizador 0.29.0→0.29.1.
- Sin hacer: ping/MMR reales en el overlay (sin fuente fiable), Auto GG, texto «BOOM» naranja de Air Strike.
- Lecciones técnicas (sonido de poste, boostmeter x4, logo `img://`, overlay transparente, PresentMon) también en `docs/PENDIENTES.md`.
