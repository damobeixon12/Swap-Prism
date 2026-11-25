# Paso 3.2: perfil local, versión 0.13.0

Primera parte del paso: editor de nombre, avatar y banner. Títulos, rango
visual y AutoGG continúan pendientes. El usuario dejó 3.1 pendiente.

## Implementación

- Fake display → Foto de perfil y nombre: texto Unicode de hasta 32 caracteres,
  selector de foto al pulsar el avatar, imagen de banner y vista previa.
  Importa PNG/JPEG/WebP, conserva el archivo original y guarda una copia PNG
  recortada dentro de data/profile/images. La vista previa no es una captura
  del juego y el logo inicial solo es un marcador de posición.
- Aplicar perfil instala una transacción de paquetes; Revertir perfil recompone
  la base cosmética anterior. Los botones están bloqueados con Rocket League
  abierto y el motor comprueba el proceso antes de preparar y de escribir.
- El nombre solo cambia los getters AS3 de presentación:
  tagame.api.LocalPlayer.PlayerName y tagame.api.PlayerInfoRow.PlayerName.
  Este último consulta bLocalPlayer y mantiene el getter original para las
  demás filas. Se conserva init(), sin escrituras en DataAPI, guardados,
  cuenta, procesos o servicios de red.
- El validador acepta únicamente los getters lineales DataAPI conocidos,
  incluyendo sus variantes con debugfile/debugline. Añade una constante de
  texto, reconstruye el tag DoABC y comprueba todas las otras funciones.
  No se promete cambiar los nombres nativos sobre los coches: la película
  GFX_Nameplates_Static no tiene estos getters. Falta validación visual real.
- Avatar y banner son swaps de Texture2D en Startup.upk:
  gfx_shared.Icon_AvatarDefault (64×64 DXT5) y
  gfx_shared.PlayerBanner_Default (420×100 DXT5). No modifican el marco.
  Son recursos COMPARTIDOS: también afectan a otros lugares/jugadores que
  utilicen esos recursos. La interfaz muestra esta limitación. Los banners
  de inventario siguen usando su categoría de Skins.
- La capa se registra en data/profile.json y conserva bases por hash en
  data/profile/bases. Una nueva textura cosmética se compone con el perfil;
  al quitar el perfil se mantiene esa nueva textura. Las copias originales
  de data/backups siguen siendo las de Rocket League, nunca las del perfil.
- Cambios realizados agrupa el perfil en una tarjeta con nombre, avatar,
  banner y reversión conjunta. Evita listar decenas de paquetes internos
  como si fueran cambios de perfil independientes. Restaurar un paquete
  cosmético también retira su capa de perfil y marca el perfil como parcial.

## Validación

- 65 pruebas Python y 10 pruebas Rust al implementar esta versión.
- work/profile-patch-check.json enumera la preparación de todos los GFX de
  la instalación real; sin escrituras en la instalación.
- work/profile_integration_check.py usa copias temporales dentro de work:
  aplica perfil, cambia el nombre a Unicode, cambia el HUD mientras conserva
  el perfil, revierte solo el perfil conservando el HUD nuevo y finalmente
  restaura los paquetes de prueba byte a byte. Verifica que los archivos
  instalados del juego no cambiaron.
- Revisión de las 23 vistas desde el framebuffer de egui en
  work/design-check-v013. Perfil y tarjeta agrupada de Cambios revisados.
- Instalador probado con /S /TESTMODE dentro de work: versión 0.13.0,
  arranque nativo y codec del motor congelado correctos, sin incluir perfiles,
  imágenes privadas o copias del usuario. Instalación temporal eliminada.
- La aplicación inicial desde el motor congelado detectó una dependencia Tk
  antigua en rl_upk_editor.py. Sus imports de GUI se hicieron perezosos, con
  anotaciones diferidas; el lector del motor ya funciona sin tkinter.
- El usuario autorizó «SWAPPRISM» y el logo para una prueba real. Se aplicaron
  64 paquetes con el juego cerrado mediante el motor distribuido y se relanzó
  desde su acceso Epic. work/profile-trial-applied.json registra esa aplicación.
  El usuario confirmó que el nombre y el logo aparecen correctamente en el
  menú principal. La verificación no se extiende al HUD, marcador, nombres
  sobre los coches ni al banner personalizado, todavía no probado.

No se ha ejecutado Shift_Setup.exe ni se han instalado proxies o certificados.
