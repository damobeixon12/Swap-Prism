# Swap Prism — contexto para Claude Code

App de Windows para personalizar Rocket League **en local**: skins, texturas, sonidos,
perfil (nombre/avatar/títulos), fake rank, mapas de Workshop, overlay de FPS y pestaña de Tracker.
Todo se hace editando los paquetes del juego (`.upk`, `.bnk`) con copia de seguridad y restauración byte a byte.
Versión actual: **0.32.0** (`app/Cargo.toml`). Repo: https://github.com/damobeixon12/Swap-Prism
(el código está en la rama main desde la 0.31.0).

Creada originalmente con Codex. El 25/11/2026 se reorganizó entera (ver «Estado de la reorganización»).

## Arquitectura (todo Rust)

```
app/ (Rust, eframe/egui 0.31)
  Swap Prism.exe = interfaz + motor (app/src/engine/, en proceso, mismo protocolo JSON)
  _engine/SWAPPRISMUpdate.exe (actualizador nativo) + _engine/licenses/
  datos del usuario en <app>/data/
```
El motor Python antiguo (engine/, PyInstaller) sigue en `main` y en la etiqueta `backup-antes-de-rust`.
- `app/src/main.rs`: arranque, `worker_progress` → `engine::handle`, modos `--smoke`, `--fps-check`, `--engine-run <req.json>`.
- `screens.rs`, `navigation.rs`, `design.rs`, `motion.rs`, `icons.rs`: interfaz (tema oscuro forzado, fuente Nunito Medium).
- `overlay.rs`, `telemetry.rs`, `platform.rs`: overlay FPS con PresentMon (`app/vendor`), ventana transparente por colorkey.
- `update.rs`, `update_runner.rs`, `update_panel.rs`: actualizaciones desde GitHub Releases (`update.json` + SHA-256).
- `tracker.rs`, `live_match.rs`: Tracker en WebView2 (Wry) y MMR del rival. `ranks.rs`, `workshop.rs`, `media.rs`, `studio.rs`, `titles.rs`, `profile.rs`.
- `engine/mod.rs`: despacho de acciones. `upk.rs`/`aes.rs` (paquetes cifrados), `state.rs` (cola, backups, restaurar),
  `apply.rs`, `jobs.rs` (catálogo embebido), `colors.rs`, `textures.rs`/`dxt.rs`, `body.rs`, `display.rs`/`hud.rs` (ActionScript),
  `profile.rs`, `studio.rs` (Wwise, MP3 con Media Foundation), `workshop.rs`, `media.rs`, `save.rs`/`game_settings.rs`, `epic.rs`, `launcher.rs`.
- `build_tool.rs`: herramienta de compilación `swapprism-build` (sustituye a los .ps1/.py).

## Estructura

| Carpeta | Qué hay | ¿En git? |
|---|---|---|
| `app/` | Rust, `installer.nsi`, `assets/` (incl. `engine/` datos y `licenses/`), `vendor/PresentMon.exe` | sí |
| `scripts/` | `swapprism-build.cmd` (prepara la toolchain y ejecuta `swapprism-build`) | sí |
| `docs/` | `PENDIENTES.md`, `historial/` (un .md por versión con detalles técnicos) | sí |
| `work/` | toolchain Rust, NSIS, cachés (`package-thumbnails`, `distribution`) | no |
| `outputs/` | instalador generado y copia de desarrollo `outputs/LocalSkins` con **datos personales** | no |

## Compilar y probar (Windows)

```powershell
scripts\swapprism-build.cmd package            # pruebas Rust + compilación + miniaturas + instalador NSIS
scripts\swapprism-build.cmd check-installer    # instala en work/installer-check y prueba el arranque (Swap Prism cerrado)
scripts\swapprism-build.cmd release            # lo que hace Preparar update.cmd (no publica nada)
```
- Pruebas de paridad con Python: `#[ignore]` en `app/src/engine/*`, con variables `SP_*` (ver cada prueba).
- Publicar: subir versión en `app/Cargo.toml` → `Preparar update.cmd` → seguir `COMO PUBLICAR ACTUALIZACIONES.txt`.

## Reglas obligatorias

- **Nunca escribir/aplicar/restaurar paquetes del juego con Rocket League abierto.** No cerrar partidas.
- No tocar `outputs/LocalSkins/data` ni `%LOCALAPPDATA%\Programs\SWAPPRISM\data` (backups, perfil, selecciones).
- No dar nada por funcionando en el juego sin confirmación del usuario; probar sobre copias no cuenta.
- No inventar IDs/nombres/imágenes de objetos. No enviar datos del guardado/perfil a servicios externos.
- No publicar Releases ni hacer push sin que el usuario lo pida.
- Pedir permiso antes de pasar a otra tarea grande; el usuario escribe en español.

## Migración a Rust (26/11/2026)

- Hecha y publicada en 0.32.0: motor y scripts en Rust, `engine/` de Python borrada (sigue en la etiqueta `backup-antes-de-rust`).
- `main` contiene ya la versión Rust (la rama `rust-engine` se unió a main).

## Pendiente de la app

Ver `docs/PENDIENTES.md`. Resumen:
- Sin confirmar en juego: fake rank (MMR 1600), color del flip reset, nombre al marcar, cambio de modelo (Body), reinicio del actualizador 0.29.0→0.29.1.
- Descartado por el usuario: ping/MMR reales en el overlay, Auto GG, texto «BOOM» de Air Strike. Siguiente: app en inglés (0.34.0).
- Lecciones técnicas (sonido de poste, boostmeter x4, logo `img://`, overlay transparente, PresentMon) también en `docs/PENDIENTES.md`.
