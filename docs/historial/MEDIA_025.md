# Swap Prism 0.25.0 — bibliotecas y descargas de cosméticos

Usuario autorizó antes de fake ranks: quitar la confirmación ZIP de Workshop,
añadir bibliotecas locales y un apartado Descargar/Cosméticos con previews para
texturas y sonidos, usando VGM, AlphaConsole y Myinstants.

Workshop descarga con un solo clic. Si Drive devuelve su formulario público de
archivo grande, se sigue dentro de la misma operación; desaparece el diálogo
extra. No se desactiva HTTPS ni se usan cuentas/cookies. Se conservan límite de
tamaño y validación ZIP/mapa, destino fijo Labs_Utopia_P.upk y aplicación con
juego cerrado. La confirmación extra anterior era de la app, no del usuario.

Texturas abre Mi biblioteca como antes, con una pestaña Cosméticos · Descargar.
Tarjetas con imágenes reales de AlphaConsole/VGM, nombre/autor cuando la fuente
lo publica, filtro por categoría, buscador y enlace a la ficha. Actualizar carga
una selección inicial de fichas públicas: 19 compatibles de la primera página
AlphaConsole y 5 VGM. NO es todo el inventario de esos sitios. VGM es una selección
inicial explícita; Añadir enlace permite incorporar otras fichas públicas de
esa fuente. Los sitios no publican aquí un contrato de API estable; parseo HTML
sin ejecutar scripts, ni autenticación, ni evasión de controles de acceso.

Fuentes: https://alphaconsole.io/browse,
https://videogamemods.com/vgm/rocketleague/mods?page=3.
ZIP se descarga a temporal y se valida/importa con el importador local existente.
Packs no compatibles informan error y no se marcan descargados. Se corrigió un
placeholder real de Sakura, Background='.png', que significa capa ausente; no
se sustituye ni inventa una imagen. Referencias normales ausentes siguen fallando.

Sonidos abre Mi biblioteca con WAV/metadata ya importados para el evento elegido.
Actual mantiene editor/recorte/volumen. Descargar consulta Myinstants:
https://www.myinstants.com/es/search/, búsqueda remota y preescucha del fragmento
permitido. MP3 se valida y convierte por miniaudio. Las preescuchas no cambian la
selección; Elegir lleva la selección a Aplicar cambios. catalog_key + kind evita
confundir un sonido preparado para poste con el mismo audio para reset. Cada
evento conserva su límite y su copia WAV. Descargas y biblioteca se guardan en
data, nunca se incluyen contenidos personales o descargados en el instalador.

Pruebas: 95 Python / 11 Rust pasan (se excluyen los 3 tests Tk antiguos sin
init.tcl, documentados previamente). check_media025.py verifica parsers de
fichas reales, descarga/importación de ambas fuentes de texturas y MP3, URLs no
permitidas y Drive con un solo clic. frozen_media025.py repite descargas reales
desde el motor empaquetado con verificación HTTPS conservada. Studio y cola
mantienen sus bytes; no se aplica ningún paquete del juego. Ninguna preescucha
automática durante esas pruebas. 39 vistas QA incluyen tienda AlphaConsole/VGM,
sonidos y biblioteca. Datos/evidencias en work/qa-media025.json y
work/frozen-media025.json. Carga nativa de cada nuevo pack no se afirma probada.

Entrega final instalada: exe/motor/setup canónicos y mismo acceso directo.
Instalador pasó arranque, codec, HTTPS y exclusión de datos privados. Revisión
nativa confirmó entrada en biblioteca, onda y preescucha que termina sin error
por el protocolo real del motor. No se afirma una escucha humana independiente.
Se dejó Descargar sonidos abierto con cola vacía. No se aplicaron demos al juego.
