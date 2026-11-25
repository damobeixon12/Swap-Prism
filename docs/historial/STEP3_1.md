# SWAPPRISM 0.12: paso 3.1 en curso

## Implementado

- Contador externo de FPS: ventana verde pequeña, transparente, sin marco,
  siempre por encima del juego y con paso de clics cuando está fijada.
- Posición arriba a izquierda/derecha o libre mediante arrastre. Preferencias
  conservadas en data/settings.json. El contador está apagado por defecto.
- Captura de eventos de presentación mediante PresentMon 2.6.0, solo del PID
  de RocketLeague.exe. No usa el FPS de SWAPPRISM, un temporizador, ni la tasa
  de paquetes de la Stats API como sustitutos del FPS del juego.
- Media móvil de intervalos de presentación de la cadena dominante. No suma
  cadenas de render distintas ni mezcla procesos. Los valores caducan tras
  1,5 segundos sin datos y se muestra — cuando no se puede medir.
- El medidor se detiene al desactivar FPS, cerrar el juego o salir de la app.
  No se inyecta código, lee memoria del juego, modifica sus paquetes, instala
  drivers ni pide elevación automática. Se desactiva el seguimiento de entrada
  de teclado/ratón y GPU que este contador no necesita.
- Funciona como overlay de escritorio; para visualizarlo sobre el juego debe
  usarse ventana sin bordes. No se promete visibilidad en fullscreen exclusivo.

Abrir Overlay > FPS y ping > Mostrar FPS. Para recolocarlo, usar Mover a cualquier
lugar, arrastrar el recuadro y pulsar Fijar posición.

PresentMon se incluye como componente privado _telemetry/PresentMon.exe, no como
otro proyecto. Se verifica antes de ejecutarlo contra el SHA-256 publicado por
GitHub para la versión oficial:
b2a706bc6ad475749e3b7e3409263aa1e6906d45bdcf993f6dbc0f660188f1af

Binario y licencia:
https://github.com/GameTechDev/PresentMon/releases/tag/v2.6.0
https://github.com/GameTechDev/PresentMon/blob/v2.6.0/LICENSE.txt

## Verificación real y límites de prueba

La prueba inicial sobre el frontend OpenGL no produjo muestras. Por eso se
creó un diagnóstico Direct3D propio, que produjo 1.196 muestras utilizables,
con 240 FPS en la última lectura. Se verificó sin permisos de administrador.
Estos son fotogramas reales de una ventana de prueba; no se presentan como
una comprobación dentro de Rocket League. El juego estaba cerrado y la prueba
real en entrenamiento sigue pendiente.

La captura del framebuffer de la ventana flotante mide 150x36 píxeles y tiene
5.182 píxeles completamente transparentes y 218 visibles. Se revisaron además
las vistas de ajustes del overlay y del estado de rangos.

66 pruebas automáticas (56 del motor/puente y 10 de Rust). También se comprobó
la parada explícita y reutilización de una sesión PresentMon propia: 288 filas
en la primera captura y 479 en la segunda; ambas y el cierre terminaron con
código 0. La app solicita cerrar su sesión antes de terminar el proceso medidor.

## Pendiente: ping y rango/MMR

El paso 3.1 no se declara terminado. El ping y el MMR de otros jugadores no
tienen todavía una fuente verificable y por eso sus controles no se activan.
No se sustituyen por ping a Internet, ping regional, datos ficticios ni tablas
de umbrales para estimar el rango de otra persona.

Investigación realizada:

1. API oficial de Rocket League:
   https://www.rocketleague.com/developer/stats-api
   Documenta jugadores y eventos de partida, pero no FPS, ping ni MMR.
   Frame y Elapsed se refieren a replays, no proporcionan un contador de FPS.
   La configuración local sigue sin tocarse (PacketSendRate=0).
2. Cliente local: el guardado actual tiene PlaylistSkillDataSave_TA con Playlist,
   Tier y MatchesPlayed propios; no el MMR de todos los jugadores. El Launch.log
   revisado tampoco contiene esos valores. No se ha leído memoria del cliente.
3. Marcador final: la actualización v2.66 documenta MMR al terminar ranked:
   https://www.rocketleague.com/news/rocket-league-patch-notes-v2-66
   Que el dato sea visible no lo convierte en un campo de la API local. Una
   lectura automática del marcador aún necesita implementación y verificación.
4. IngameRank (MIT), propuesto por el usuario:
   https://github.com/BenTheDan/IngameRank
   Rankings.cpp obtiene GetPlayerRank/GetPlayerMMR de MMRWrapper en BakkesMod.
   El propio autor indica que el plugin deja de ser utilizable con EAC y archivó
   el repositorio. Su geometría puede ser una referencia; no ofrece un proveedor
   externo de MMR. No se ha ejecutado ni instalado el plugin ni copiado su código.
5. trn-rocket-league (MIT), propuesto por el usuario:
   https://github.com/romain-dlt/trn-rocket-league
   fetchProfile.js ejecuta curl contra api.tracker.gg. El repo está archivado;
   no ofrece una API propia. Una petición normal al ejemplo público del README
   (PannH.) devolvió 403; no se intentó sortearlo.
   https://feedback.tracker.gg/t/rocket-api-non-existent-why/56516
   El personal de Tracker afirma que no ofrece una API de Rocket League y
   remite a Psyonix. No se integra scraping de ese endpoint como fuente estable.

Una petición con el nombre leído del registro local fue bloqueada por revisión
automática de privacidad antes de ejecutarse. No se enviaron datos de la cuenta;
se comprobó el endpoint con el ejemplo que publica el autor del repositorio.

El usuario no necesita jugar otra partida ni mantener abierto el marcador para
la prueba técnica de FPS. No se ha pasado al paso 3.2 ni al arreglo del boost.

## Actualización de investigación — 22 noviembre 2026

El usuario propone estudiar de dónde obtiene Tracker el rango, diferenciando
API de Rocket League frente a servicios de Epic. Investigación pública, sin
consultas con identidades del usuario ni uso de tokens del cliente.

- El personal de Tracker declara que usa las API de Rocket League únicamente
  en sus proyectos y remite a Psyonix para obtener autorización propia:
  https://feedback.tracker.gg/t/rocket-league-public-api/9214/2
- Restricciones por acuerdos con desarrolladores:
  https://feedback.tracker.gg/t/open-the-rl-api/32388
- Respuesta de febrero de 2026: Tracker no ofrece una API pública de RL:
  https://feedback.tracker.gg/t/regarding-on-development-api/65636
- La Stats API local oficial sigue documentando telemetría de partido por
  socket local; no contiene campos Rank/MMR en el esquema consultado:
  https://www.rocketleague.com/developer/stats-api?lang=en
- EOS Stats configura datos del producto propio en su despliegue, con sus
  clientes y políticas. Inferencia: registrar Swap Prism como producto no
  concede acceso automático al MMR de un producto ajeno como Rocket League:
  https://dev.epicgames.com/docs/epic-online-services/player-and-game-data/stats-interface/stats-guide/set-up-stats

La propuesta tiene fundamento: un backend online con acceso autorizado es
distinto de la API local de telemetría. No se ha encontrado ni integrado una
API pública que entregue esos rangos para Swap Prism. No se contactó a nadie.

Las secciones históricas anteriores describen el estado de la 0.12. La app
ya está en 0.16; FIX_016.md documenta interfaz/perfil/boostmeter entregados.
El hallazgo de GFxShell como biblioteca compartida durante esa corrección
queda registrado en AGENTS.md; las antiguas sondas de ping/MMR siguen retiradas.

### Nueva comparación solicitada: IngameRank y peticiones a Tracker

Revisión directa de Rankings.cpp: updateRankFor obtiene GetMMRWrapper(),
comprueba IsSynced y usa GetPlayerRank/GetPlayerMMR. IngameRank.cpp registra
RegisterMMRNotifier para actualizar el dibujo. No hay una petición HTTP a
Tracker en esa obtención de rangos. El README declara la dependencia de
BakkesMod y el archivo del proyecto con la llegada de EAC.
https://github.com/BenTheDan/IngameRank/blob/main/IngameRank/Rankings.cpp
https://github.com/BenTheDan/IngameRank/blob/main/IngameRank/IngameRank.cpp

Se hizo UNA petición HTTP GET ordinaria al ejemplo PannH. publicado por el
autor de trn-rocket-league, sin cookies, tokens, identidades del usuario ni
imitación de navegador. Respondió HTTP 403. No se reintentó ni se intentó
eludir la restricción. Esa prueba no proporciona una fuente operativa.

Inspección nueva SOLO LECTURA de GFxShell.CodeLibrary: informes
work/rank-codelibrary-apis.json y work/rank-codelibrary-disassembly.txt.
PlayerInfo.GetSkillMMR/GetbMMRWasReplicated leen filas de DataAPI; Playlists
incluye GetSkillMMR y UpdateSkillData, este último llama a GFxData_Playlist_TA
mediante SHELL_CallUnrealFunction. Leaderboards tiene GetMMR de su conjunto
de filas. Son APIs internas de interfaz, NO endpoints HTTP ni una prueba de
que se pueda consultar el MMR de cualquier rival durante el partido.
No se invocaron esas funciones, no se modificó el juego y no se capturaron
rangos reales en esta investigación. La ruta de obtención sigue pendiente.
