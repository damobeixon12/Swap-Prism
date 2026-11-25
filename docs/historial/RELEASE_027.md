# Distribución, actualizaciones, Discord y limpieza · 0.27.0

El usuario pidió reducir el proyecto, generar instalador y actualización,
preparar un flujo para publicar nuevas versiones y añadir Discord. Facilitó
explícitamente https://github.com/damobeixon12/Swap-Prism.git.

Canal estable configurado en los valores por defecto incorporados:
https://github.com/damobeixon12/Swap-Prism/releases/latest/download/update.json
Las instalaciones antiguas con canal vacío adoptan el valor nuevo, manteniendo
las demás preferencias. Una respuesta 404 del canal todavía sin publicar se
trata como ausencia de versiones disponibles. No se ha publicado nada.

El usuario ejecuta Preparar update.cmd después de incrementar app/Cargo.toml.
work/prepare_release.ps1 sincroniza Cargo.lock y versión NSIS, compila, calcula
SHA-256 y crea outputs/update.json y outputs/Swap Prism-Update.zip. El ZIP
contiene exclusivamente Swap-Prism-Setup.exe y update.json, listos para subir a
una Release estable con etiqueta vX.Y.Z. No crea otra aplicación LocalSkins.
La app descarga el instalador y comprueba hash, HTTPS y versión. El helper
install-update.ps1 espera a que termine Swap Prism, verifica de nuevo el hash
original y abre el instalador en la misma carpeta, evitando la carrera del mutex.
Un push del código no publica por sí solo una versión instalable.

El workflow opcional .github/workflows/update-manifest.yml genera el manifiesto
cuando se publica una release estable con instalador; comprueba la versión del
EXE. Es un archivo local preparado: no se ha subido ni ejecutado en GitHub.
El repositorio y los assets deben ser públicos para clientes sin autenticación.
Guía práctica: COMO PUBLICAR ACTUALIZACIONES.txt en la raíz.

Discord tiene sección propia, diseño discreto y enlace al invite autorizado
https://discord.gg/dXa24Y5HtX. Abrir el enlace no confirma adhesión al servidor.

La limpieza autorizada retiró 7.735 GiB de paquetes generados, targets de Rust,
fuentes/cachés descargables de Cargo, duplicados de compilación, ZIP de herramientas
ya extraídas y pruebas temporales. El proyecto quedó en aproximadamente 3.4 GiB;
LocalSkins en 1.118 GiB y el instalador en 130.2 MiB. Se mantienen las herramientas
necesarias para recompilar y todas las bibliotecas/originales/selecciones.
Se verificaron SHA-256 de los 10588 archivos de data antes y después: intactos.
work/cleanup027-report.json conserva el inventario y las rutas eliminadas.
work/cleanup_project027.ps1 valida raíces, rechaza enlaces y exige app/motor
cerrados. generated_cache.py retira los payloads temporales después de un commit
exitoso, conservando los informes, para evitar acumular varios GB otra vez.

Validación: 95 pruebas Python, 16 Rust y prueba específica del descarte seguro de
generados. Instalador 0.27.0 probado con arranque, codec, HTTPS y exclusión de datos
privados. 42 vistas de QA. ZIP inspeccionado: solo dos assets; manifiesto con el
hash real del setup y URL de la versión 0.27.0. No se ha simulado que exista una
release remota ni probado actualización desde un servidor aún sin publicar.

Se incluye la corrección de MMR interno (visual-100)/20 y CODEC_REVISION=7 de
0.26.1. El usuario retiró el fake rank durante el trabajo; un intento de instalar
la preparación anterior se rechazó por selección cambiada. Se conserva su última
configuración sin fake rank. No se reaplicó la demo ni se verificó runtime 1600.
