# Swap Prism 0.23.0 — Workshop y movimiento

Usuario autorizó el catálogo inspirado en su captura, animación de despliegue
de categorías y selección que viaja hacia Aplicar cambios. Aclaración posterior:
destino exclusivamente Utopia Retro / Balón rueda, sin selector para cambiarlo.

Se identificó Labs_Utopia_P.upk, presente en la instalación y documentado por
los creadores de mapas en https://dev.rocketleaguemapmaking.com/cheatsheet/quick_testing.
El motor rechaza otro destino aunque llegue en una petición manual.

Sección Workshop: tarjetas con imágenes reales, buscador, orden por descargas
o nombre, filtro descargados, importación ZIP/UDK/UPK, descarga e indicador de
activo/pendiente. Catálogo público Lethamyr: 204 mapas y 204 imágenes verificadas
y cacheadas durante la entrega. No representa todos los mapas de Steam. El
endpoint es público observado, no una API con contrato de estabilidad.
No se usa ni se ejecuta Shift ni se desactiva TLS.

Los ZIP grandes de Drive que muestran aviso de análisis requieren confirmar
en la app. No se usan cuentas ni cookies. Si la fuente no entrega un ZIP válido,
se informa del error y queda disponible Importar. No se ejecutan archivos
descargados. Solo se acepta un mapa principal, se comprueba cabecera Unreal y
tamaño, se rechazan rutas ZIP inseguras, enlaces simbólicos y archivos enormes.
La cabecera no demuestra compatibilidad runtime ni resuelve dependencias de
texturas o mutadores de cada mapa.

Elegir/Restaurar original pasan por la cola superior; última selección Workshop
sustituye la anterior. Se conserva el original verificado, se aplica mediante
la transacción existente y solo con el juego cerrado. La biblioteca es local
y separada del mapa instalado. Se impide borrar de ella el mapa activo.

Categorías usan CollapsingState animado de egui. La selección genera una etiqueta
de 650 ms que sigue una curva hacia el botón superior, se reduce y desvanece.
También cubre perfil y personalización. Quitar una selección no genera vuelo;
se limita la frecuencia durante cambios continuos de texto/color.

Pruebas: 95 Python y 11 Rust pasan. Tres tests antiguos de la interfaz Tk no
pudieron arrancar por falta de init.tcl en el runtime actual y se excluyeron del
segundo pase; la app Rust no usa Tk. Harness propio comprueba destino fijo,
deduplicación, rechazo de traversal, protección del mapa activo y aplicación /
restauración byte exactas de un mapa descargado real SOBRE COPIAS en workspace.
work/qa-workshop023.json; work/frozen-workshop023.json verifica motor empaquetado.
Triple Goal descargado/importado como prueba de transporte. No se ha instalado
en Rocket League ni se afirma haberlo jugado.

34 vistas QA, incluidas Workshop a 1280 y 1060 px. UI nativa observada: catálogo,
filtro Descargados, Triple Goal, Elegir → Aplicar cambios (1), retirada de selección
→ (0). Sin escribir paquetes del juego. Exe e instalador canónicos actualizados;
instalador probado con arranque/codec y exclusión de datos privados. Acceso
directo existente conservado. Las últimas selecciones del usuario, incluido el
audio confirmado, no se reaplicaron ni restauraron.
