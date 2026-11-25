# Instrucciones para asistentes (Claude, Codex…) en Swap Prism

Lee primero `README.md` (estructura y compilación) y `docs/PENDIENTES.md` (estado y lecciones).
El historial completo de decisiones está en `docs/historial/`.

## Reglas del usuario

- Trabajar siempre sobre este proyecto. No crear copias ni ejecutables alternativos.
- Hacer solo lo autorizado y pedir permiso antes del siguiente paso.
- **Nunca escribir, aplicar ni restaurar paquetes del juego con Rocket League abierto.** No cerrar una partida en curso.
- Conservar `outputs/LocalSkins/data`, los originales verificados y las selecciones más recientes del usuario. No volver a aplicar demos antiguas.
- No inventar IDs, nombres ni imágenes de objetos.
- No dar una función por verificada en el juego sin confirmación del usuario. Una prueba sobre copias no es una prueba en el juego.
- No enviar a servicios externos datos del guardado o del perfil sin permiso explícito. No usar tokens ni credenciales del juego ni de otras apps (Shift, Tracker).
- Publicar en GitHub Releases es un paso explícito que decide el usuario.
- Usar el logo original de `app/assets/logo.png`.

## Arquitectura en una línea

App Rust (`app/`) → lanza `_engine/SWAPPRISMEngine.exe` (motor Python de `engine/`, empaquetado con PyInstaller) con `SWAPPRISM_ROOT` = carpeta de la app → datos en `data/`.

## Compilación

`scripts/package_swapprism.ps1` (pruebas + Rust + motor + instalador); `Preparar update.cmd` para publicar.
Pruebas del motor: `python -m unittest discover -s engine/tests -t engine`.

## Instalaciones del usuario

- Instalada: `%LOCALAPPDATA%\Programs\SWAPPRISM` (la que abre el acceso directo).
- Desarrollo: `outputs/LocalSkins` (con su propio `data/`). Ambas tienen copias de seguridad propias.
