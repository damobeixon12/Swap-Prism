# Swap Prism 0.24.0 — navegación y legibilidad

Alcance: pulir desplegables, más movimiento discreto, Personalización por
subcategorías, tipografía redondeada clean y texto mayor. El usuario pidió
conservar la animación hacia Aplicar: motion.rs no se ha modificado.
Fake ranks queda como siguiente paso, sin implementación en esta entrega.
Usuario confirmó explícitamente que descargar mapas en 0.23.1 funciona.

Nueva navegación con filas de identidad estable, estados de foco/hover suaves,
indicador activo, flecha que gira y grupos con altura conocida, easing cúbico,
deslizamiento corto, opacidad y guía vertical. El usuario puede cerrar el grupo
activo sin abandonar su página. Se sustituyó CollapsingState, cuyo primer
fotograma estimaba altura. Los controles ocultos no admiten clic durante la
transición. Navegación lateral ampliada de 224 a 260 px.

Personalización ya no duplica pestañas horizontales. Cinco hijos con iconos:
paleta, sonido del reset, poste/travesaño, color del reset e inicio de temporada.
Cada sonido tiene página propia con los mismos controles y selecciones previas.
No se modifican MP3, bancos de audio, paquetes, inventario ni opciones del juego.

Fuente Nunito incorporada, instancia estática peso 500 para evitar que ab_glyph
use el mínimo 200 de la variable. SIL OFL incluida en el motor/instalador.
Origen https://github.com/google/fonts/tree/main/ofl/nunito. Generada localmente
con fontTools (herramienta de desarrollo, no dependencia distribuida). Segoe UI
de Windows sirve de fallback para flechas/símbolos. Texto base 16, botones 15,
small 13, títulos 28 px, navegación principal 15 / hijos 14. Algunos textos
dibujados explícitamente se ampliaron también. Sin cambios a preferencias FPS.

Transición de página de 220 ms (fade suave y desplazamiento 7 px), respuesta
del contorno de tarjetas al hover y transiciones de estados de navegación.
Los movimientos solicitan repaint solo mientras están activos.

11 tests Rust pasan. Se renderizaron 35 vistas de QA, incluidas dimensiones
pequeñas, tarjetas, títulos y ambos sonidos separados. Se corrigió durante QA
el peso fino de la variable y símbolos ausentes. Revisión nativa: abrir
Personalización, seleccionar Poste/travesaño y cerrar el grupo conserva página
y selección; cola sigue en cero. Rocket League permaneció abierto; ninguna
escritura a sus paquetes. Artefactos work/design-check-v024-final.

Entrega canónica completada con instalador verificado (arranque, codec, HTTPS y
sin datos privados), mismo acceso directo y aplicación abierta en Personalización.
