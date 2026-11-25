# Pendientes

Estado a 0.29.1. Detalle técnico de cada punto en `docs/historial/`.

## Hecho, pero sin confirmar dentro del juego

- **Fake rank**: corrección del MMR visual (antes mostraba 32100 en vez de 1600). `RANK_TRACKER_026.md`, `FIX_0261.md`.
- **Color del flip reset** y **nombre propio al marcar / en la repetición** (0.20). `FIX_020.md`.
- **Cambio de modelo (Body)** con dependencias visuales del donante, para evitar coches invisibles. `UPDATE_017.md`.
- **Actualizador nativo**: reinicio final de 0.29.0 → 0.29.1. `UPDATER_029.md`.

## Sin hacer

- **Ping y MMR reales en el overlay**: no hay todavía una fuente de datos verificable. `STEP3_1.md`, `SHIFT_TRACKER_REVIEW.md`.
- **Auto GG**.
- **Texto «BOOM» naranja de Air Strike**: material compartido `Boost_Tests.Materials.Boom_Text_MIC`; investigado, sin corregir.

## Lecciones que no hay que repetir

- Sonido de poste: el nodo de impacto 772249675 lo comparten las 20 superficies. Solo funciona el aislamiento de la versión 023, que requiere un índice DIDX ordenado, datos alineados a 16 bytes en dirección absoluta y los hijos HIRC antes del selector padre. `FIX_0211.md`, `FIX_022.md`.
- Boostmeter: hay 4 copias de `BoostMeterView` (HUD, CodeLibrary, repetición, espectador). Editar solo el HUD no basta. `FIX_016.md`.
- Logo de inicio: `ImageLoader` ya añade `img://`; no anteponerlo. `FIX_019.md`.
- Overlay transparente: necesita `WS_EX_LAYERED` + colorkey negro en su HWND; `with_transparent` no basta.
- PresentMon: no usar `no_track_display` / `no_track_gpu` (dan 0 muestras).
- Avatar: solo el propio, añadiendo un fotograma nuevo en `TintedAvatarIcons`. No reemplazar `gfx_shared.Icon_AvatarDefault` de forma global.
