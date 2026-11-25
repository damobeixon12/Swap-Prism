# SWAPPRISM: paso 1

Este directorio pertenece al proyecto original. No es una segunda aplicación.
La aplicación que se abre es outputs/LocalSkins/SWAPPRISM.exe. El nombre de esa
carpeta se conserva para mantener las rutas de las copias originales verificadas.

## Arquitectura

- Rust/egui: ventana, navegación, búsquedas, imágenes, cola persistente, detección
  del juego, F8, instancia única y actualización. El catálogo está incluido en el
  ejecutable; el motor no bloquea el primer dibujo de la ventana.
- _engine/SWAPPRISMEngine.exe: componente privado Python empaquetado en carpeta,
  sin interfaz Tk ni extracción onefile. Lee el guardado y aplica las operaciones
  existentes de skins/texturas/restauración en segundo plano mediante JSON ASCII.
  No necesita Python ni Rust instalados en el equipo del usuario.
- Se mantienen las comprobaciones originales de integridad, copias de seguridad,
  compatibilidad y bloqueo de escrituras mientras Rocket League está abierto.
- data/queue.json conserva también los pendientes entre aperturas.
- El instalador incluye las miniaturas públicas, pero no inventarios, guardados,
  copias del juego ni packs personales. Los amigos importan sus propios packs.

El nombre y el logo proporcionados se usan en la ventana, el icono PE, el
instalador y el acceso directo. El rediseño del Canva queda para el paso 2.

## Detección y rutas

Se comprueba la ruta guardada, los manifiestos de Epic y las rutas habituales de
Epic/Steam de las unidades C a Z. La selección manual pide exactamente la carpeta
rocketleague, con Binaries/Win64/RocketLeague.exe y TAGame/CookedPCConsole.
La detección de ejecución se realiza con el listado de procesos de Windows.

## Compilar y distribuir

Ejecutar work/package_swapprism.ps1 desde la raíz del proyecto. Usa Rust y
LLVM-MinGW portátiles dentro de work/toolchain y el Python del runtime de Codex.
Cargo.lock fija las versiones resueltas. NSIS se obtuvo de una copia cuya huella
SHA-256 coincide con la publicada por el proyecto oficial.

El resultado compartible es outputs/SWAPPRISM-Setup.exe. Instala por usuario en
LOCALAPPDATA/Programs/SWAPPRISM, permite elegir carpeta y crea un acceso directo.
No solicita permisos de administrador ni instala controladores. El instalador
informa de que no tiene firma digital. Si SWAPPRISM está abierto, pide cerrarlo.
No instala dependencias externas: el motor y sus bibliotecas van incluidos.

Las actualizaciones conservan data y el desinstalador también conserva esa carpeta.
Para dejar el juego original, restaurar desde la aplicación antes de desinstalar.
El desinstalador no toca los archivos del juego.

## Preparación de actualizaciones (sin publicar)

Por petición del usuario, no se ha creado repositorio ni publicado ninguna URL.
El canal permanece vacío: no se contacta con un servidor ficticio al abrir la app.

Para activar el canal antes de distribuir una compilación pública:

1. Publicar el instalador en una URL HTTPS accesible.
2. Calcular su SHA-256 con Get-FileHash.
3. Publicar un JSON conforme a update-manifest.example.json, con versión SemVer,
   URL HTTPS del EXE, SHA-256 real y notas opcionales.
4. Configurar update_manifest_url en app/assets/settings.json antes de empaquetar.
   Las instalaciones ya existentes pueden configurarlo en data/settings.json.
5. Probar desde una versión anterior. Se anuncia únicamente una versión estable
   mayor: «HAY UNA ACTUALIZACIÓN PENDIENTE, actualiza para ver los nuevos cambios».
6. El usuario decide descargar y después cerrar/instalar. Se verifica SHA-256,
   se rechazan HTTP, hashes inválidos, descargas excesivas y bajadas de versión.

El SHA-256 detecta alteraciones respecto al manifiesto HTTPS; no sustituye una
firma del editor. No se ha afirmado que exista una actualización publicada ni se
ha ejecutado un instalador remoto para probar este mecanismo.

## Comprobaciones

54 pruebas del motor/puente y 3 de Rust. Prueba de arranque nativo contra el
inventario real y diagnóstico de instalación limpia. Las pruebas antiguas de
Tk se mantienen como referencia del frontend anterior y no forman parte de la
validación de la interfaz Rust; el runtime actual no tiene Tcl configurado.
La automatización visual de Windows no pudo iniciarse (CreateProcessWithLogonW
1920); queda pendiente la revisión visual interactiva. No se probaron nuevas
apariencias dentro del juego como parte de este paso.
