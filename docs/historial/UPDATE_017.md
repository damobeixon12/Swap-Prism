# Swap Prism 0.17 — personalización

Petición del usuario: referencias visuales de Shift y Discord, con identidad
propia. Se conserva el logo original, el proyecto, data y las selecciones previas.
No se ejecutó Shift_Setup.exe. Esta entrega no implementa proveedores de MMR.

## Interfaz y FPS

Tema grafito, superficies delimitadas, inicio con arena vectorial propia,
accesos y navegación con iconos. Personalización reúne paleta, sonidos, reset
y temporada. Títulos tiene biblioteca de los IDs reales leídos del inventario,
editor personalizado y título activo; no se atribuyen nombres o colores oficiales
que no se hayan leído. Se conservan las opciones de texto/glow/multicolor.

Overlay externo transparente: número seguido de FPS, verde por defecto, tamaño
10–28 px, color, cuatro esquinas y arrastre. Corregida posición derecha para
pantallas que no comienzan en x=0 y altura para que el tamaño mayor no recorte.
Corrección posterior solicitada: la configuración OpenGL de la ventana principal
también solicita transparencia, porque todos los viewports comparten esa elección.
Antes el framebuffer del contador tenía alpha cero pero la ventana nativa podía
aparecer negra. Ese primer ajuste NO bastó en este equipo. La solución final
añade WS_EX_LAYERED/LWA_COLORKEY negro SOLO al HWND del contador cuyo PID coincide
con Swap Prism; no afecta a Rocket League ni a Shift. También se retira el relleno
negro del modo de arrastre. Tras reinstalar/reabrir la app, el usuario confirmó:
«Sí, ahora solo se ve el texto». Esta confirmación sustituye una inferencia basada
únicamente en el PNG interno, que ya tenía alpha pero no demostraba el resultado
del compositor de Windows.
`work/fps017-live.json`: PresentMon obtuvo 1195 muestras, última 240 FPS del
proceso RocketLeague.exe. No se leyó memoria del juego. El ping sigue pendiente.

## MP3

Se identificó Play_Car_Movement_FlipReset mediante su hash de evento y árbol
HIRC. Las dos variantes originales duran 29571/48000 = 0.6160625 segundos.
Poste/travesaño se identificaron desde PhysicalMaterialProperty_TA.AkSwitchValue
Goalpost_Post/Goalpost_Crossbar hasta la rama metálica del evento Impact_Ball.
Son 18 variantes, máximo 2.01175 segundos; permanece el contacto compartido de
la pelota. No se sustituye el salto normal ni todos los impactos del juego.

`app/assets/audio_manifest.json` conserva hashes de los bancos, IDs obtenidos
de ellos y posiciones verificadas de los campos fuente. Una versión incompatible
se rechaza. MP3 de hasta 8 MB, duración limitada según evento, decodificación
local a PCM mono 48 kHz, forma de onda, nombre, volumen y preescucha. Sin subidas
a servidores. Los archivos originales no se redistribuyen.

Los bancos reconstruidos conservan cada medio no seleccionado y cada sección
ajena. Solo se actualiza el códec/tamaño en las fuentes seleccionadas y el índice
de sus medios. Se comprobó la decodificación PCM con vgmstream, herramienta usada
solo en investigación. wwiser se utilizó para interpretar los árboles Wwise;
ninguna de esas herramientas se distribuye con la app. Miniaudio está empaquetado.

## Colores, temporada y coches

La paleta modifica exclusivamente Colors de CarColors.BlueTeamV3/OrangeTeamV3
(70 colores cada una) y CarColors.Accent (105, compartida). Se mantienen tamaños,
índices y funciones del juego. Se elige un color/casilla en la app y después
se equipa desde el selector nativo del garaje. No promete acentos independientes
por equipo: el juego comparte ese conjunto de colores.

Reset: solo LUT RGB y sus límites de ParticleModuleColor dentro de FX_FlipReset.
Requiere tener activado el efecto visual nativo. No modifica duración ni detección
del reset. La composición de temporada sustituye la textura Logo_Evergreen de
GFX_StartMenu por logo/nombre/Season N propios; conserva otras texturas y créditos.
El número es parte del diseño, no cambia la temporada real de Rocket League.

El puente antiguo de carrocería importaba mallas de otro paquete seek-free, que
podía no estar cargado y producía un coche invisible. La nueva generación conserva
las dependencias visuales del donante dentro de su paquete, renombra el asset
y trasplanta los campos físicos del coche original. Normaliza FNames/referencias,
verifica HandlingPreset y cada campo físico, y elimina offsets físicos del donante
ausentes en el original. Una dependencia física no resoluble se rechaza.
Preparados y comprobados: Fennec→Homer, Fennec→Octane, Octane→Fennec y
Fennec→Dominus. No se afirma haber probado todos los pares en el juego.

## Verificación y límites

- 79 pruebas Python y 10 Rust superadas.
- 32 vistas renderizadas con el frontend real.
- `work/check_studio017.py`: MP3 válidos, rechazo de exceso de duración,
  paleta/reset y bancos aplicados/restaurados byte a byte en copias de trabajo;
  textura de temporada reconstruida y legible.
- `work/check_frozen_audio017.py`: decodificador MP3 y preescucha silenciosa
  comprobados desde el motor empaquetado, con estado de prueba aislado.
- `work/body017-check.json`: campos físicos y modelo incorporado comprobados.

Los guards de juego cerrado permanecen activos en producción. El harness de
copias simula presencia únicamente para sus destinos temporales dentro de work;
no escribe la instalación real. Rocket League estaba abierto durante la entrega:
no se aplicaron las selecciones nuevas a sus archivos. Sonidos, reset, paleta,
logo de temporada y aspecto de coches requieren comprobación visual/auditiva
dentro del juego tras elegirlos y aplicar con Rocket League cerrado. No se
presentan esas pruebas de archivo como una comprobación runtime.
