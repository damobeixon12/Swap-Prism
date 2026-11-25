# SWAPPRISM 0.11: rediseño según el Canva

Se mantiene el mismo proyecto, el ejecutable principal y el acceso directo.
La navegación principal contiene los seis apartados solicitados: Skins,
Texturas, Fake Display, Overlay, Cambios realizados y Ajustes. El logo abre Inicio.

- Inicio: bienvenida con el nombre del inicio de sesión de Rocket League que
  corresponde a la cuenta del guardado; si no hay nombre verificable, muestra
  solo Bienvenido. Carrusel animado de cinco tarjetas con imágenes reales de
  catálogo/biblioteca y funciones futuras identificadas como Próximamente.
- Skins: segundo menú de categorías, nombre del objeto elegido arriba,
  inventario a la izquierda y nueva apariencia a la derecha, búsquedas y fotos.
  Se conservan el equipo, color, cola y comprobaciones del motor. Las categorías
  de consulta no permiten añadir cambios.
- Texturas: segundo menú Boostmeter/Pelota/Decals; cuadrícula adaptable con
  una imagen real y botón Añadir por pack, búsqueda e importación de archivos
  o carpetas. Para decals sigue siendo obligatorio elegir el original poseído.
- Fake Display: Rank, Foto de perfil y nombre, Títulos. Solo estructura visual;
  no cambia nombres, fotos, títulos ni rangos en este paso.
- Overlay: FPS y ping, Rank in game, Soon. Solo estructura visual; no muestra
  métricas ficticias ni implementa un overlay en este paso.
- Cambios realizados: pendientes, instalados y restaurados con imágenes,
  estado y restauración individual/completa. Se conservan los paquetes y backups.
- Ajustes: ruta del juego, inventario completo/refresco, versión y estado del
  canal de actualizaciones. El canal sigue sin publicar.

Paleta oscura con acentos cian, violeta y verde. Tarjetas y botones de esquinas
suaves, sin contornos. Aplicar/añadir en verde; quitar/revertir en rojo. La fuente
Segoe UI se carga desde Windows y dispone de las fuentes de respaldo de egui.

## Verificación

56 pruebas del motor/puente y 6 de Rust. 23 capturas obtenidas mediante el
framebuffer del propio renderizador egui, con datos reales y sin escrituras en el
juego: las pantallas principales, subcategorías futuras y vistas a 1280x860 y
1060x740. Se revisaron las capturas y se corrigieron ancho de tarjetas,
alineación de filas/acciones y glifos de los indicadores del carrusel.

El modo de desarrollo --design-check genera imágenes locales; no pulsa Añadir,
Aplicar o Revertir ni cambia las preferencias del usuario. Sustituye la limitación
de revisión visual documentada para el paso 1; no requiere automatizar Windows.

El paso 3.1 (overlay) espera la confirmación del usuario. No se ha retomado
la investigación del boost naranja ni se han cambiado sus algoritmos.
