---
version: 1
slug: "templates-portfolio-html"
primary_target: "templates/portfolio.html"
related_targets: ["src/routes/api/portfolio.rs","src/routes/web/portfolio.rs","static/js/portfolio.js","static/css/portfolio.css"]
---

Mode: Operate. Visitante: usuario LatAm multi-moneda que revisa su cartera cada tanto (hoy: el dueño, ES primero). Superficie: `/inversiones` — medir la estructura del portafolio contra la estrategia escrita, no operar ni dar recomendaciones de compra/venta.

Scope: cartera persistida en PostgreSQL; valores cargados a mano en ARS o USD; tasas MEP/CCL desde DolarAPI; cuatro vistas de valuación; perfiles conservador/moderado/agresivo como presets editables; activos de renta fija, renta variable y mixtas con split prorrateado; 11 chequeos, score, rebalanceo y disparadores de venta. Sin IOL, sin credenciales de broker, sin cotizaciones automáticas todavía. Sin capturas del md original: el producto no lleva material de terceros.

## Direction contract

THESIS: la misma prensa de cartel del home, puesta a medir una cartera. El diagnóstico se estampa, no se dibuja con un anillo: un sello de goma inclinado −6° dice el score; el resto son barras de cartel, chequeos reglados y filas de libro. Rechaza el dashboard fintech de tarjetas redondeadas y gauges circulares.

OWN-WORLD: los cinco materiales del sistema (papel, tinta, rojo óxido, ocre, concreto) siguen haciendo trabajo semántico. Renta fija = concreto (ancla); renta variable = tinta (motor); cíclico = rojo; defensivo = concreto; mixto = tinta-2. El rojo queda para déficit, alerta, venta y la acción primaria; el ocre para el positivo y la compra. Cero radio, bordes duros 2–3px, sombra solo bajo controles elevados, movimiento steps(2) o corte instantáneo. Las diagonales son las tres del mundo: clip de banda 26px, cuña −30°, sello −6°.

STORY: el visitante elige su perfil y su vista (ARS/USD a MEP/CCL), carga lo que tiene con su tesis y sus disparadores, y lee tres cosas: si el ancla y el motor están en objetivo, qué bloque se concentra de más, y qué activo se salió de la banda. Desde la orden puede ajustar el activo al objetivo; desde un disparador cumplido puede venderlo completo y cerrar el aviso. La cartera de ejemplo se trae solo con un botón explícito cuando está vacía, y se advierte que es editable y no toca cuentas ni libro.

FIRST VIEWPORT (1440×900): shellbar tinta con INVERSIONES en rojo actual; franja de corte con selector de vista y tasas MEP/CCL con hora de lectura; sello de diagnóstico a la izquierda; bloque de mando con el total en Anton, la moneda de la vista, el detalle de estado y cuatro KPIs (activos, economías, unidades efectivas, renta fija); debajo, la banda roja de Diagnóstico y la primera fila de chequeos.

FORM: sistema gráfico constructivista (Lissitzky/Rodchenko) ya comprometido en el home. Raises propias: **stamp-not-gauge** (el score es un sello, no un anillo), **ledger-not-cards** (chequeos, disparadores y órdenes son filas regladas), **one-rate-per-view** (todas las posiciones se convierten con la misma tasa elegida; no se mezclan MEP y CCL en una misma lectura), **no-silent-zero** (si falta la tasa, la superficie dice "sin valuar" y no inventa 0), **demo-on-demand** (la cartera de ejemplo nunca se siembra sola), **pending-outside-panel** (los disparadores cumplidos de todos los activos viven en un bloque propio, no dentro del activo elegido), **position-not-price** (el campo es el valor total de la posición; la tabla muestra también el equivalente en la vista).

FINISH: el build cierra con fmt, clippy sin warnings, tests del motor y del API (rebalanceo, ejecución de disparador y vaciado), y esta ficha de superficie. La deuda conocida es de fase 2: cotizaciones automáticas (solo BYMA Open Data) y la liquidez como contraparte automática de ventas/compras.

## Decisiones tomadas

- Cuatro vistas: `ARS · MEP`, `ARS · CCL`, `USD · MEP`, `USD · CCL`. La tasa se lee de DolarAPI y se cachea 10 minutos; si la fuente falla se mantiene la última lectura marcada como vieja.
- Perfiles conservador 70/30, moderado 55/45, agresivo 40/60; banda y límites por preset. Todo editable: si el usuario se desvía, la UI muestra `PERSONALIZADO`.
- Mixta: `fixed_income_pct` por activo; el motor prorratea el valor entre RF y RV. Economía y sector son una etiqueta única (sin look-through).
- El chequeo de integridad valida que los pesos objetivo sumen 100; los pesos actuales se derivan de los valores, no se cargan a mano.
- La liquidez es un activo de renta fija marcado con `ES LIQUIDEZ` (uno por usuario, sin disparadores). Es la contraparte: las ventas la acreditan y las compras la debitan. Se excluye de las órdenes por activo y de los chequeos de disparadores/tesis.
- `EJECUTAR` en una orden de rebalanceo ajusta el activo al valor objetivo y mueve la diferencia contra la liquidez; el total no cambia y el peso queda exacto. Las órdenes se ejecutan de a una, como en la vida real; no hay rebalanceo global. `MARCAR EJECUTADO` vende el activo completo, lo saca de la cartera, acredita la venta en la liquidez y resuelve el disparador.
- El chequeo de cantidad cuenta **solo activos de renta variable** (8–13); renta fija, liquidez y mixtas no entran. La tarjeta muestra el reparto igual de la RV (60 % ÷ n) y, si hay mixtas, el número con la mixta fija.
- El chequeo de pesos objetivo muestra cuánto falta o sobra para llegar a 100 % y pide ajustar uno o más objetivos.
- El panel `OBJETIVOS` aparece cuando la RV o la RF se desvían del perfil. Reparte solo porcentajes (no compra ni vende), con modos partes iguales / mantener proporciones y preview antes de aplicar. La mixta y la liquidez quedan fijas.
- Orden: Cartera → Rebalanceo → Cuándo vender → Ajustes. El bloque `A EJECUTAR` está fuera del panel del activo y tiene `VER ACTIVO` y `MARCAR EJECUTADO`.
- `Ancla y motor` muestra la banda y los pp fuera; su chip pasa a `FUERA DE BANDA` en el escalón intermedio. La leyenda bajo Diagnóstico explica BIEN / FUERA DE BANDA / ALERTA.
- `VACIAR CARTERA` aislado y con confirmación. El ejemplo se carga solo desde el estado vacío, con la nota de que es editable y no toca cuentas ni libro; sus pesos objetivo respetan el 40/60.
- Fase 2: BYMA Open Data como única fuente de cotizaciones, cuando la rueda está activa (delay ~20 min), con caché de la última lectura para fines de semana y feriados. DolarAPI ya está integrada para MEP/CCL; no se usa data912 ni se conecta IOL.
- BYMA: uso No profesional mientras RFinance sea personal/self-hosted; si se publica como producto o se redistribuyen los datos, pasa a Profesional y requiere licencia.

## Decisiones abiertas

- Si la vista ARS debe usar MEP o CCL por defecto (hoy MEP para ARS y CCL para USD; el usuario puede cambiar).
- Liquidez: un activo marcado `ES LIQUIDEZ` por usuario; `/balances` del broker queda para fase 3.
- Ejecución exacta al 10 %: con liquidez marcada, el activo queda exactamente en su peso objetivo y el total no cambia. Sin liquidez, el ajuste usa el valor objetivo y el peso queda dentro de la banda (±5 pp), con el total recalculado.
- Cotizaciones automáticas: mapear tickers y resolver el `quote_unit` de bonos/ONs (por 100 nominales) antes de encenderlas.
- i18n EN: la copia de esta superficie está en español; el mecanismo bilingüe sigue pendiente.
