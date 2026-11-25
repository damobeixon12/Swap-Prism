# Prueba controlada del MP3 solo en poste/larguero

**RETIRADA:** el usuario confirmó que 022 también dejaba muda la pelota. Se
restauró el original byte a byte con el juego cerrado. Alinear y ordenar DIDX
no bastó. No reinstalar el hash 357cfed4….

El usuario aclaró que el audio original funciona y quiere oír su MP3 únicamente
en poste/larguero. Se corrigieron las dos condiciones que fallaban en 0.21:

- DIDX se ordena por ID y se comprueban duplicados/límites de cada muestra.
- Se ajusta el padding de BKHD para conservar alineación ABSOLUTA de DATA y de
  todas las muestras. El banco nativo SFX_Car_Movements ya contiene padding en
  BKHD; no se añaden índices falsos ni se cambian campos funcionales de cabecera.

Resultado del candidato: DATA comienza en 9744 (múltiplo de 16), índice ordenado,
806 muestras alineadas, 0 desalineadas. El segundo lector reconoce padding=8.
Se conservan las 788 muestras originales y las 18 superficies no-post; solo las
dos entradas Goalpost_Post/Goalpost_Crossbar se redirigen a las copias de audio.
93 pruebas Python pasan, incluidas regresiones de índice y alineación absoluta.
work/qa022-post.json y work/post022-candidate contienen el candidato exacto.

El usuario confirmó de nuevo Rocket League cerrado. Se instaló SOLO este banco
mediante transacción/guard normal; work/installed-post022-candidate.json conserva
el hash 357cfed46a76943e4970b413649c853b9eec328ac67c0bbf57f29616a11a2b5b.
Se relanzó con su URL Epic para comprobación en entrenamiento.

La aplicación pública sigue en 0.21.1 y la opción de poste permanece bloqueada.
No se ha publicado ni declarado confirmada esta variante. Está pendiente la
respuesta auditiva del usuario: solo poste/larguero personalizado y resto normal.
Si falla, restaurar el original únicamente con el juego cerrado, antes de otra
entrega. No confundir este candidato con el banco roto de 0.21, hash c7ffb02b….

Prueba posterior 023: el original serializa los 965 enlaces hijo→padre sin
referencias adelantadas. 022 introducía 9. Se conserva ahora el orden original
de los objetos clonados y se insertan antes del selector padre: 987 enlaces,
0 referencias adelantadas, DIDX ordenado y 0 muestras desalineadas. Está en
work/post023-candidate, validada por check_post023.py, hash 78beb044748b1958cf6b99f7436a5d92916ca028b08fbe4886adffa53a555608.
Se instaló un solo banco con el juego cerrado y se relanzó por Epic; pendiente
comprobación del usuario. Esto reproduce una condición del original, NO prueba
por sí solo que sea la causa del rechazo ni que funcione en el cliente.
