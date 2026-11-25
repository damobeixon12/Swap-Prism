# Investigación de catálogo de mapas — 2026-11-23

Alcance autorizado: investigación. No se implementa una pantalla, descarga un
mapa ni modifica un paquete de Rocket League. No se ejecuta Shift.

## Fuente comprobada directamente

https://lethamyr.com/api/v1/maps?limit=1 respondió HTTP 200 JSON con validación
TLS normal. `limit=1` NO limita la respuesta: devuelve 25 elementos, paginación
`links.next` y `meta.total=204`, `last_page=9`. No confundir esos 204 mapas con
todo Steam Workshop. Evidencia local: work/workshop-catalog-research.json.

Cada elemento observado contiene name, description_short, description,
download_url, counts y media (URLs de imágenes). Ejemplo real Triple Goal con
enlace de descarga Google Drive. La ficha pública https://lethamyr.com/maps/1
también ofrece descarga. Solo se comprobó catálogo/enlace; no se descargó ni
validó el ZIP o su funcionamiento dentro del juego.

Es un endpoint público observado, NO una API documentada con garantía de
estabilidad ni autorización genérica de redistribución. Una integración debería
respetar atribución del autor, enlaces originales, caché y límites del sitio;
revisar condiciones o consultar al responsable para uso sostenido. Es viable
empezar con fichas e importación local y descargar solo desde fuentes comprobadas.

## Steam y otras fuentes

Valve documenta GetPublishedFileDetails, POST con itemcount y publishedfileids,
sin key listada como parámetro requerido. QueryFiles sí requiere key. Son vías
de metadatos; no demuestran por sí solas descarga anónima de cualquier mapa.
https://partner.steamgames.com/doc/webapi/ISteamRemoteStorage
https://partner.steamgames.com/doc/webapi/IPublishedFileService

rl-map-manager usa catálogo GitLab en celab.jetfox.ovh/api/v4, releases ZIP y
metadatos de Steam. Su README afirma ~730 mapas, pero NO se verificó esa cifra
ni la disponibilidad actual: una consulta HTTPS normal agotó 18 segundos.
Su backend desactiva validación TLS; no copiar ese comportamiento. Tampoco
adoptar sin comprobar sus afirmaciones sobre Workshop read-only desde 2020.
https://github.com/KillianM00/rl-map-manager
https://raw.githubusercontent.com/KillianM00/rl-map-manager/main/backend.py

El código actual de Vyncc incluye instalador de mapas desde
bakkesplugins.com/maps y ZIP en cdn.bakkesplugins.com. La página devolvió 403
al navegador de investigación; no se sorteó ni se verificó catálogo/descarga.
https://github.com/Vyncc/Workshop-Map-Loader-Downloader
https://raw.githubusercontent.com/Vyncc/Workshop-Map-Loader-Downloader/master/Pluginx64/BakkesPluginsMapInstaller.cpp

## Carga en Epic y propuesta para Swap Prism

La FAQ del creador documenta que Custom Map Loader sustituye Underpass y se
selecciona Underpass en freeplay. Eso fundamenta el mecanismo de sustitución,
no que cualquier estadio o mapa funcione. Otros mapas pueden necesitar paquetes
de texturas o ajustes particulares. No se verificó cómo obtiene su catálogo Shift.
https://lethamyr.com/faq

Propuesta pendiente de autorización: sección Workshop con tarjetas (imagen,
nombre, autor cuando lo exponga la fuente, descripción y procedencia), buscador,
biblioteca descargada e importación ZIP/UDK/UPK. Selección del entrenamiento a
reemplazar limitada a destinos comprobados; empezar por Underpass. Descargar a
data, validar archivo y dependencias; poner el cambio en la cola superior.
Aplicar/restaurar con el juego cerrado, originales verificados y transacción
existente. No prometer todos los mapas ni todo estadio como destino sin pruebas.
