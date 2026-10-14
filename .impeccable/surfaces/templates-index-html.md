---
version: 1
slug: "templates-index-html"
primary_target: "templates/index.html"
related_targets: ["templates/login.html","templates/register.html"]
---

Mode: Operate. Visitante: usuario LatAm multi-moneda (hoy: el dueño, ES primero). Superficie: `/` (index.html) — tablero + libro en una sola página. Related: `/login`, `/register` (mundo visual idéntico; formularios y endpoints intactos).

Scope: shell visual con datos sintéticos etiquetados "DE EJEMPLO"; sin migraciones ni persistencia; formulario simula en memoria con firma "El Sello". Sin dependencias nuevas. Fuentes self-host (static/fonts). Texturas raster procedurales (static/textures) con provenancia embebida.

## Direction contract

THESIS: un escritorio de trabajo del dinero en pesos y dólares, compuesto como una prensa de cartel constructivista. Rechaza el default de la categoría: grilla de tarjetas fintech redondeadas sobre crema con un acento; acá mandan bloques de tinta planos, grid rígido de columnas y sellos.

OWN-WORLD: cinco materiales con trabajo semántico — hueso envejecido (fondo; grano raster real), rojo bandera-óxido (egreso, déficit, acción primaria, bandas), negro tinta (estructura y reglas), gris concreto (paneles secundarios), ocre (ingreso/positivo). Tipografías: Saira Stencil One (marca y títulos de sección), Anton (numerales y botones en mayúsculas), JetBrains Mono variable (todo dato, tabular). Bordes duros 2px, cero radio, sombra dura solo bajo controles elevados; diagonales únicamente en bandas de sección; movimiento step(2) a 90 ms por fotograma, sin easing; selección, foco, caret y scrollbar tematizados en tinta/rojo.

STORY: el visitante lee su total por moneda, anota un movimiento en tres pasos y lo ve estamparse en el libro; presupuestos y carteles-estadísticos dicen la verdad del mes; todo está etiquetado DE EJEMPLO hasta que llegue el backend. Creen: instrumento de registro, no app juguete. Vuelve mañana y anota rápido.

FIRST VIEWPORT (1440×900): barra shell tinta: RFINANCE stencil a la izquierda; nav de secciones en mayúsculas (deshabilitadas, "en construcción"); usuario + logout a la derecha (aislado, outline hasta foco). Banda de mando: dos bloques ARS | USD con numerales Anton ~96px, neto del mes + histograma real 6 meses; banda diagonal roja como rótulo de sección; formulario de carga rápida adosado a la derecha de la banda, primario "REGISTRAR". Debajo: tira de cuentas; luego el libro contable.

FORM: sistema gráfico del cartel constructivista (Lissitzky/Rodchenko), candidato propio #3 por asignación del roll (key 1667472d). seed: none (unavailable). Raises nombradas — manual-acetate: toda transición de estado es un corte duro de dos fotogramas (90 ms), nunca fade; dark-console: la acción destructiva vive aislada, en outline hasta foco deliberado; saville-catalog: campo vastísimo sin imprimir alrededor de cada bloque denso; ikeda-datamatics: márgenes de la banda de mando llenos de columnas mono de datos; zoo-guide-map: un solo sistema de ángulo de corte gobierna diagonales, barras del gráfico y el sello; doujin-catalog (competitive): la grilla del libro recompacta en celdas completas sin columnas huérfanas.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Decisiones abiertas

- Ingreso en ocre (elección actual) vs. verde: cambio de un token si el dueño pide.
- i18n: ES ahora; mecanismo EN queda abierto (strings ya centralizados en templates).
- Esquema real de finanzas (tablas/monedas/conversión): llega con el backend; no inventar tasas.
- admin.html: placeholder fuera de scope; recibe el mundo vía style.css, contenido intacto.
