# Actualizador integrado 0.29.1

La prueba 0.27 → 0.28 descargó y verificó el instalador, pero el antiguo
lanzador PowerShell no lo abrió con ExecutionPolicy=Restricted. No se cambió
esa política. La instalación no se dio por completada.

La nueva pantalla de la app muestra el logo original, búsqueda, versión
disponible y progreso de descarga con bytes reales. «Actualizar ahora» verifica
SHA-256 e inicia el componente privado nativo SWAPPRISMUpdate.exe. La app
espera que este dibuje su ventana antes de cerrarse. El componente instala
en silencio en la misma carpeta, muestra progreso basado en los archivos
extraídos por NSIS y vuelve a abrir Swap Prism. No usa PowerShell ni cambia
configuraciones de seguridad. Los errores permanecen visibles.

Windows requiere cerrar brevemente el ejecutable para sustituirlo; durante
ese relevo permanece la ventana con logo y progreso del actualizador.

Publicación estable:
https://github.com/damobeixon12/Swap-Prism/releases/tag/v0.29.1

Instalador: 43 429 481 bytes (41,42 MiB). SHA-256:
cb6225a65e808d94edfe3a97d383bcc87c3c9da9025ee7d6e538178b515cdfca

Los clientes 0.27/0.28 necesitan instalar esta corrección manualmente una vez,
porque su lanzador antiguo no puede arreglarse desde el propio binario cerrado.
Las versiones siguientes utilizan el lanzador nativo.

Validación: 17 pruebas Rust de la app y 5 del componente; 95 Python;
instalación limpia con arranque, codec, HTTPS y exclusión de datos privados;
preescucha del motor empaquetado sin importar el sonido en la biblioteca.
Los cambios de tema y miniaturas de 0.28 se conservan.

Prueba interactiva preparada: cliente instalado 0.29.0 frente a la versión
real 0.29.1 publicada. Resultado final pendiente de pulsar «Actualizar ahora»
y comprobar reinicio, log, hashes y conservación de los 9221 archivos previos.
No confundir la preparación del instalador con esta prueba completa.

Se mantiene el acceso directo a LOCALAPPDATA/Programs/SWAPPRISM. El ejecutable
del proyecto sigue en outputs/LocalSkins/Swap Prism.exe, con el mismo motor.
No se escribieron paquetes de Rocket League.
