# Revisión local de Shift — 22 noviembre 2026

El usuario pidió inspeccionar el Shift ya instalado y abierto. Se revisó
Shift 2.7.1 mediante la habilidad computer-use actualizada y lecturas locales.
No se ejecutó Shift_Setup.exe, no se cambiaron ajustes del tracker ni paquetes
del juego. Solo se navegó a Tracker > Settings/Match y se dejó Shift minimizado.

## Observación directa

- Tracker > Settings separa un overlay de sesión y un MMR scoreboard overlay.
- El del marcador está configurado como Hold to show, tecla Tab.
- Opciones separadas Show rank, Show division, Show playlist y Show MMR.
- La escala de UI y la escala de pantalla se pueden calibrar por separado.
- La interfaz indica funcionamiento en Borderless/Windowed, no Fullscreen.
- Tracker > Match mostró Champion I, 1084, modalidad 2v2, con una fila propia
  y sin rivales en la sesión de entrenamiento observada. No se verificó una
  partida online con rivales ni se contrastó ese rating con el servidor.
- Se observó también una ventana flotante de sesión con 1084 MMR.

## Evidencia técnica local

- El proceso Shift tiene una conexión TCP local establecida a 49123.
- Sus logs registran activación automática de DefaultStatsAPI.ini, MatchCreated,
  MatchInitialized y consulta de perfiles para el roster al reconocer partida.
- El tracker conserva puntos de inicio de sesión/MMR separados por modalidad.
- Los registros contienen etiquetas [psynet] y mensajes de skills fetch/patch.
- Literales del ejecutable incluyen una dirección propia de bridge en Render
  y una plantilla de enlace a perfiles web de Rocket League Tracker. Estas
  cadenas solas NO demuestran que Tracker sea el proveedor ni que el bridge
  ofrezca una API pública reutilizable.

No se copiaron tokens, claves ni credenciales, ni se consultó un servicio externo
con las identidades del usuario/jugadores. La inspección de cadenas de URLs se
limitó a metadatos sin query ni credenciales. No se capturó/decriptó tráfico ni
se leyó memoria del juego. Los logs privados no se copiaron a este informe.

## Conclusión y límite

La arquitectura observable combina telemetría local para contexto de partida,
obtención de perfiles para los ratings y una ventana externa activada con Tab.
Es compatible con separar esas tres funciones en Swap Prism. La procedencia
exacta y las condiciones de acceso a los ratings todavía requieren verificación;
no se ha integrado el servicio privado de Shift ni implementado MMR en Swap Prism.

La anterior afirmación de que computer-use no iniciaba ya está superada para
esta sesión: la versión actual de la habilidad sí permitió observar Shift.

## Prueba online posterior: 1v1 con consulta de 2v2

El usuario inició una partida competitiva de 1v1 dejando 2v2 seleccionado en
Shift. Observación directa del marcador con Tab durante el partido (reloj
4:32): dos filas, propio 1084 y rival 710 entre corchetes, insignias 2v2/rango
a la izquierda. El título personalizado, avatar propio y medidor corregido
también eran visibles, sin necesidad de tocar sus ajustes.

Las dos entradas nuevas [psynet] del registro de Shift al iniciar ese partido
coinciden con 2v2/1084 y 2v2/710. Informe numérico SIN nombres/IDs en
work/shift-live-mmr-observation.json. No se enviaron inputs al juego, no se
cambió de ventana durante la partida ni se escribieron archivos de Rocket League.

Esto confirma que Shift presenta ratings de una modalidad elegida distinta
de la partida actual antes del final. NO verifica por sí solo la exactitud
del rating contra un proveedor independiente ni identifica sus credenciales,
protocolo completo o condiciones de uso. La fuente de ratings sigue pendiente
de esclarecer antes de integrarla en Swap Prism. No se usaron sus servicios
privados ni se instaló un proveedor nuevo en nuestra aplicación.
