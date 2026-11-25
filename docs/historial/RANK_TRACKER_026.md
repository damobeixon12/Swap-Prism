# Fake rank y Tracker · 0.26.0

El editor de Perfil y títulos permite elegir modalidad, MMR, rango y división,
ver una muestra y añadir la selección a Aplicar cambios. La última selección
sustituye a la anterior. Se puede retirar solo el rango sin perder nombre,
avatar o título. El juego debe estar cerrado para aplicar o restaurar.

La modificación afecta a la presentación de la modalidad seleccionada en el
menú competitivo: getters de Playlists y PlaylistsRow, condicionados por el Id
nativo de la modalidad. No cambia rango del servidor, matchmaking ni rango de
otros jugadores. CODEC_REVISION=6 mantiene la composición con otras capas.
La escala automática es una simulación visual explícita (1300 → GC I,
1900 → SSL); no representa los umbrales reales por modalidad. También admite
selección manual. La presentación dentro del juego todavía requiere prueba
runtime al aplicar la selección del usuario: no se ha aplicado una demo.

Tracker muestra el sitio oficial en un WebView2 integrado, con alternativa
Abrir en navegador. El usuario facilitó y autorizó su nombre Epic;
se conserva el punto final. Se verificó visualmente que su perfil carga y
muestra rangos, y que cambiar a Perfil y títulos oculta la vista web. No se
consultan APIs privadas, no se extraen credenciales y no se calculan MMR reales.
La web puede mostrar publicidad, verificaciones o datos con retraso. No implica
integración de rangos en el marcador del juego. Tracker declaró que no ofrece
API pública de Rocket League:
https://feedback.tracker.gg/t/regarding-on-development-api/65636

WebView2 utiliza el runtime de Microsoft instalado y WebView2Loader.dll junto
al exe. Configuración y caché del navegador permanecen bajo data y se excluyen
del instalador; el nombre del usuario no se incorpora a la distribución.

Validación: 95 pruebas Python, 14 Rust y 41 vistas QA. El harness independiente
verifica 24 getters, conservación de otras modalidades y métodos, y aplicación
y reversión byte a byte sobre copias. El motor congelado confirma ambas copias
de la biblioteca. Registros: work/qa-fake-rank026.json y
work/frozen-rank026.json. Ningún paquete del juego fue escrito en esta entrega.

Instalador 0.26.0 probado: arranque, codec, HTTPS verificado y exclusión de
datos privados, incluida la caché del Tracker. Exe y setup canónicos actualizados.
