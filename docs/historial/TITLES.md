# Swap Prism 0.14 — títulos personalizados

La marca visible, ventana, ejecutable, instalador y acceso directo son
**Swap Prism**. Se conserva el mismo directorio de datos, el identificador de
actualizaciones/registro y el mutex anterior para que una actualización no
cree una segunda instalación. Los identificadores privados del motor no son
otro proyecto ni una segunda aplicación.

## Comportamiento

Fake display → Títulos ofrece un editor de hasta 48 caracteres Unicode,
colores RGB libres, segundo color para degradado, color de resplandor y
activación/desactivación del resplandor. La vista previa muestra el avatar,
nombre y título debajo, siguiendo el Canva, y reduce la fuente al escribir
frases largas. La fuente y el glow de la vista previa son orientativos.

El selector usa los TitleId de atributos reales de productos del inventario,
excluyendo blueprints, más el título equipado confirmado por el guardado.
Se muestran los códigos como códigos, sin inventar nombres localizados.
El usuario debe equipar el título elegido para ver su sustitución. No se
escribe el guardado ni se concede/equipa un título mediante la aplicación.

Añadir y aplicar título se hace con el juego cerrado, en una transacción.
Revertir título conserva nombre, avatar, banner y capas cosméticas actuales.
La tarjeta de Cambios realizados muestra el título y permite editarlo o
revertirlo. El botón principal permanece visible fuera del panel desplazable.

## Implementación y alcance

- PlayerTitlesPlayerTitlesRow.Text/Color/GlowColor comparan Id con el título
  original escogido; otros títulos mantienen el getter original.
- PlayerInfoRow.XPTitle/TitleColor/TitleGlowColor solo se sustituyen para el
  jugador local cuando el texto original coincide con GetTitleData del título
  escogido. Si el motor todavía no proporciona ese objeto durante la carga,
  se mantiene la lectura original. GetTitleData, SetPlayerTitle y los datos
  de cuenta no se modifican.
- Los getters nuevos mantienen pila, scopes y métodos restantes. Los colores
  se convierten a uint; las cadenas se añaden al pool conservando los índices.
- Multicolor usa FONT COLOR hexadecimal en htmlText de LocalPlayerView y
  TitleListItemRenderer. El texto libre escapa caracteres especiales con
  entidades numéricas para soportar el toUpperCase del renderizador nativo.
  No se ejecuta HTML arbitrario proporcionado por el usuario.
- El resplandor real usa PlayerTitleStyleManager y su GlowFilter ya existente
  (blur 10, fuerza 2, calidad media). Las paletas son editables e inspiradas
  en estilos, no una afirmación de hexadecimales oficiales.
- GFX_MidGameScoreboard no incluye la API PlayerTitles necesaria para esta
  comprobación contextual. No se promete cobertura del marcador/HUD sin
  observarla. Los nombres nativos sobre los coches tampoco se han probado.

## Evidencia

- 70 pruebas Python y 10 Rust; pruebas de entrada, escape, Unicode y color.
- work/title-routing-check.json prueba los getters editados contra una DataAPI
  simulada: título escogido, títulos diferentes, otros jugadores, título
  equipado diferente y objeto de título todavía nulo durante carga.
- work/title-patch-check.json valida la preparación mono/multicolor de todos
  los paquetes GFX reales, sin escribir la instalación.
- work/profile-integration-check.json añade título multicolor al nombre/avatar
  en copias temporales y revierte solo el título byte a byte. También prueba
  una nueva textura del HUD y la restauración completa de los archivos de
  prueba. Comprueba que el juego instalado permanece intacto durante ese test.
- Demo real: «SWAP PRISM ON TOP», verde, sobre el título equipado XP_Superstar.
  Se aplicaron 64 paquetes con el juego cerrado mediante el motor congelado,
  conservando nombre y avatar. Después se relanzó por el acceso Epic.
- work/title-startup-check.json confirma proceso activo, menú principal y
  ausencia de VerifyError, errores AS3 o errores fatales en el registro.
  **Esto no demuestra la apariencia en pantalla.** La herramienta de control
  visual no inicia por CreateProcessWithLogonW 1920, incluso tras reset.
- 26 vistas revisadas desde el framebuffer propio de la app, incluyendo el
  título verde, degradado, frase larga y ventana de 1060×740. Se suavizó la
  previsualización del glow y se fijaron Aplicar/Revertir fuera del scroll.
- El motor congelado prepara también el paquete multicolor con los recursos
  JSON de operandos incluidos (work/title-frozen-smoke.json), sin tocar el juego.
- Instalador 0.14 probado en carpeta temporal con /S /TESTMODE: arranque y
  codec correctos; sin perfiles, imágenes privadas o backups del usuario.
  La carpeta temporal se eliminó. El acceso directo es Swap Prism.lnk y se
  retiraron los ejecutables principales e instaladores con el nombre anterior.

## Fuentes primarias

- Epic: GC rojo y SSL blanco:
  https://www.epicgames.com/help/c-37599050/c-32343914/rocket-league-a18023509
- Psyonix, recompensas de títulos:
  https://www.rocketleague.com/news/competitive-season-1-title-rewards?lang=en
- Autodesk, texto parcial con FONT COLOR hexadecimal:
  https://help.autodesk.com/cloudhelp/ENU/ScaleformStudio-Help/scaleform_studio_help/getting_started/using_fonts/fonts_editor/html_text.html
- Autodesk, uso y coste de htmlText:
  https://help.autodesk.com/cloudhelp/ENU/Scaleform-Help/scaleform_help/best_practices/content_creation.html
- Métodos y estructuras comprobados en los paquetes locales del usuario,
  mediante lectura y desensamblado sin ejecución de ActionScript.
