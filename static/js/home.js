// RFinance — home: cliente del API con render client-side.
// El estado financiero vive en la base de datos (scope del user_id de
// sesión); el GET / materializa las recurrencias y este módulo pinta
// todo del GET /api/estado. Las mutaciones pasan por la API JSON.
// El Sello (alta), el corte duro steps(2) y los materiales del mundo
// quedan a cargo de style.css / dashboard.css.

(function () {
	'use strict';

	// ---------- utilidades ----------

	var nfARS = new Intl.NumberFormat('es-AR', { maximumFractionDigits: 0 });
	var nfUSD = new Intl.NumberFormat('es-AR', { minimumFractionDigits: 2, maximumFractionDigits: 2 });

	var MESES = ['ENE', 'FEB', 'MAR', 'ABR', 'MAY', 'JUN', 'JUL', 'AGO', 'SEP', 'OCT', 'NOV', 'DIC'];
	var MESES_LARGOS = ['ENERO', 'FEBRERO', 'MARZO', 'ABRIL', 'MAYO', 'JUNIO', 'JULIO', 'AGOSTO', 'SEPTIEMBRE', 'OCTUBRE', 'NOVIEMBRE', 'DICIEMBRE'];
	var reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

	function fmt(valor, moneda) {
		return (moneda === 'USD' ? nfUSD : nfARS).format(valor);
	}

	function montoTxt(valor, moneda) {
		return (valor < 0 ? '−' : '') + fmt(Math.abs(valor), moneda) + ' ' + moneda;
	}

	function fechaCorta(iso) {
		return iso.slice(8, 10) + ' ' + MESES[Number(iso.slice(5, 7)) - 1];
	}

	function mesIso(anio, mes) {
		return anio + '-' + String(mes).padStart(2, '0');
	}

	function el(tag, clase, texto) {
		var n = document.createElement(tag);
		if (clase) n.className = clase;
		if (texto !== undefined) n.textContent = texto;
		return n;
	}

	function svg(tag, atributos) {
		var n = document.createElementNS('http://www.w3.org/2000/svg', tag);
		for (var k in atributos) n.setAttribute(k, atributos[k]);
		return n;
	}

	function $(id) {
		return document.getElementById(id);
	}

	// ---------- API ----------

	async function pedir(url, opciones) {
		var r = await fetch(url, opciones);
		if (r.status === 401) {
			window.location.href = '/login';
			throw new Error('sesión expirada');
		}
		return r;
	}

	async function cargarEstado() {
		var r = await pedir('/api/estado');
		if (!r.ok) throw new Error('el servidor no contesta el estado');
		estado = await r.json();
		if (!vistaMes) vistaMes = mesIso(estado.anio, estado.mes);
	}

	// ---------- estado ----------

	var estado = null; // del GET /api/estado
	var vistaMes = null; // 'YYYY-MM' del mes visto en libro y presupuestos
	var anulados = {}; // ids en ventana DESHACER (borrado diferido)
	var timersUndo = {};
	var orden = { clave: 'fecha', dir: -1 };
	var editarMvId = null;
	var editarEsDeRec = false; // la fila en edición pertenece a una recurrencia
	var editarCuentaId = null;
	var editarCatId = null;

	function activo(m) {
		return !anulados[m.id];
	}

	function esMesVisto(iso) {
		return iso.slice(0, 7) === vistaMes;
	}

	function esFuturo(iso) {
		return estado && iso > estado.hoy;
	}

	// serie anual ENE→DIC del año en curso, derivada de los asientos reales
	function serieDelAno(moneda) {
		var serie = [null, null, null, null, null, null, null, null, null, null, null, null];
		if (!estado) return serie;
		for (var i = 0; i < estado.movimientos.length; i++) {
			var m = estado.movimientos[i];
			if (!activo(m) || m.moneda !== moneda) continue;
			if (Number(m.fecha.slice(0, 4)) !== estado.anio) continue;
			var mes = Number(m.fecha.slice(5, 7)) - 1;
			var par = serie[mes] || [0, 0];
			if (m.valor > 0) par[0] += m.valor;
			else par[1] += -m.valor;
			serie[mes] = par;
		}
		return serie;
	}

	function mostrarEstado(texto, clase) {
		var aviso = $('capturaEstado');
		aviso.textContent = texto;
		aviso.classList.toggle('aviso-error', clase === 'aviso-error');
		aviso.classList.toggle('aviso-exito', clase === 'aviso-exito');
		aviso.hidden = false;
	}

	// ---------- banda de mando ----------

	function renderMando() {
		var porMoneda = { ARS: 0, USD: 0 };
		var nombresPorMoneda = { ARS: [], USD: [] };
		for (var i = 0; i < estado.cuentas.length; i++) {
			var c = estado.cuentas[i];
			porMoneda[c.moneda] += c.saldo;
			nombresPorMoneda[c.moneda].push(c.nombre);
		}
		$('totalArs').textContent = fmt(porMoneda.ARS, 'ARS');
		$('totalUsd').textContent = fmt(porMoneda.USD, 'USD');
		$('totalArs').closest('.mando-total').classList.toggle('total-neg', porMoneda.ARS < 0);
		$('totalUsd').closest('.mando-total').classList.toggle('total-neg', porMoneda.USD < 0);
		$('subArs').textContent = nombresPorMoneda.ARS.join(' · ') || 'sin cuentas ARS';
		$('subUsd').textContent = nombresPorMoneda.USD.join(' · ') || 'sin cuentas USD';

		$('histoAno').textContent = 'AÑO ' + estado.anio;
		var serie = serieDelAno('ARS');
		var netoAnual = 0;
		for (var m = 0; m < 12; m++) {
			if (serie[m]) netoAnual += serie[m][0] - serie[m][1];
		}
		$('netoAnual').textContent = netoAnual ? montoTxt(netoAnual, 'ARS') + ' NETO' : 'SIN DATOS';
		var caja = $('mandoHisto');
		caja.innerHTML = '';
		caja.appendChild(graficoAnual(serie, 'histo', 'Ingresos y egresos mensuales en pesos, año ' + estado.anio, caja.clientWidth));
	}

	function graficoAnual(serie, claseCss, etiquetaAria, W) {
		W = Math.max(320, Math.round(W || 660));
		var H = 240;
		var maximo = 0;
		for (var m = 0; m < 12; m++) {
			if (serie[m]) maximo = Math.max(maximo, serie[m][0], serie[m][1]);
		}
		var g = svg('svg', { class: claseCss, viewBox: '0 0 ' + W + ' ' + H, role: 'img', 'aria-label': etiquetaAria });
		var pitch = W / 12;
		var base = H - 32;
		for (var m2 = 0; m2 < 12; m2++) {
			var x = m2 * pitch + pitch * 0.18;
			var par = serie[m2];
			if (par) {
				barra(g, x, par[0], maximo, 'var(--ocre-barra)', MESES[m2] + ' ingresos: ' + fmt(par[0], 'ARS'), base);
				barra(g, x + pitch * 0.36, par[1], maximo, 'var(--rojo)', MESES[m2] + ' egresos: ' + fmt(par[1], 'ARS'), base);
			} else {
				g.appendChild(svg('rect', { x: x, y: base - 2, width: Math.max(8, pitch * 0.18), height: 2, fill: 'var(--concreto)' }));
				g.appendChild(svg('rect', { x: x + pitch * 0.36, y: base - 2, width: Math.max(8, pitch * 0.18), height: 2, fill: 'var(--concreto)' }));
			}
			var t = svg('text', { x: m2 * pitch + pitch / 2, y: H - 12, 'text-anchor': 'middle' });
			t.textContent = MESES[m2];
			g.appendChild(t);
		}
		g.appendChild(svg('line', { class: 'eje', x1: 4, y1: base, x2: W - 4, y2: base }));
		return g;
	}

	function barra(grafico, x, valor, maximo, color, titulo, base) {
		var h = Math.max(3, Math.round((valor / maximo) * (base - 16)));
		var rect = svg('rect', { x: x, y: base - h, width: 12, height: h, fill: color });
		var t = svg('title');
		t.textContent = titulo;
		rect.appendChild(t);
		grafico.appendChild(rect);
	}

	// ---------- cuentas ----------

	function renderCuentas() {
		var grilla = $('cuentasGrilla');
		grilla.innerHTML = '';
		for (var i = 0; i < estado.cuentas.length; i++) {
			var c = estado.cuentas[i];
			var bloque = el('div', 'cuenta' + (c.moneda === 'USD' ? ' cuenta-malla' : ''));
			bloque.appendChild(el('span', 'rotulo', c.nombre));
			bloque.appendChild(el('span', 'cuenta-saldo' + (c.saldo < 0 ? ' cuenta-neg' : ''), fmt(c.saldo, c.moneda)));
			bloque.appendChild(el('span', 'cuenta-chip', c.moneda));
			var acciones = el('div', 'cuenta-acciones');
			acciones.appendChild(boton('EDITAR', function (cta) { return function () { entrarEdicionCuenta(cta); }; }, c));
			acciones.appendChild(boton('BORRAR', function (cta) { return function () { borrarCuenta(cta); }; }, c, 'btn-mini-peligro'));
			bloque.appendChild(acciones);
			grilla.appendChild(bloque);
		}
		$('estadoCuentas').hidden = estado.cuentas.length !== 0;
		sincronizarSelects();
	}

	function boton(texto, fabrica, dato, extra) {
		var b = el('button', 'btn btn-mini' + (extra ? ' ' + extra : ''), texto);
		b.type = 'button';
		b.addEventListener('click', fabrica(dato));
		return b;
	}

	async function mutar(url, metodo, cuerpo, avisoExito) {
		var opciones = { method: metodo };
		if (cuerpo !== undefined) {
			opciones.headers = { 'Content-Type': 'application/json' };
			opciones.body = JSON.stringify(cuerpo);
		}
		var r = await pedir(url, opciones);
		if (!r.ok) {
			var texto = await r.text();
			mostrarEstado(texto || 'No se pudo guardar', 'aviso-error');
			throw new Error(texto);
		}
		await cargarEstado();
		renderTodo();
		if (avisoExito) mostrarEstado(avisoExito, 'aviso-exito');
		return r;
	}

	async function borrarCuenta(c) {
		var n = 0;
		for (var i = 0; i < estado.movimientos.length; i++) {
			if (activo(estado.movimientos[i]) && estado.movimientos[i].cuenta === c.nombre) n++;
		}
		var msg = '¿Borrar la cuenta «' + c.nombre + '»?';
		if (n) msg += ' Sus ' + n + ' asiento(s) del libro también se borran.';
		if (!window.confirm(msg)) return;
		try {
			await mutar('/api/cuentas/' + c.id, 'DELETE', undefined, 'Cuenta «' + c.nombre + '» borrada.');
		} catch (e) { /* el aviso ya quedó en pantalla */ }
	}

	function entrarEdicionCuenta(c) {
		editarCuentaId = c.id;
		$('altaTitulo').textContent = 'EDITAR CUENTA';
		$('altaNombre').value = c.nombre;
		$('altaMoneda').value = c.moneda;
		$('altaSaldo').value = c.saldoInicial;
		$('altaCuentaBtn').textContent = 'GUARDAR';
		$('altaCuentaCancelar').hidden = false;
		$('altaNombre').focus();
	}

	function salirEdicionCuenta() {
		editarCuentaId = null;
		$('altaTitulo').textContent = 'AGREGAR CUENTA';
		$('altaCuentaForm').reset();
		$('altaCuentaBtn').textContent = 'AGREGAR';
		$('altaCuentaCancelar').hidden = true;
	}

	$('altaCuentaForm').addEventListener('submit', async function (e) {
		e.preventDefault();
		var cuerpo = {
			nombre: $('altaNombre').value.trim(),
			moneda: $('altaMoneda').value,
			saldo_inicial: parseFloat(($('altaSaldo').value || '0').replace(',', '.')),
		};
		try {
			if (editarCuentaId !== null) {
				await mutar('/api/cuentas/' + editarCuentaId, 'PATCH', cuerpo, 'Cuenta «' + cuerpo.nombre + '» guardada.');
				salirEdicionCuenta();
			} else {
				await mutar('/api/cuentas', 'POST', cuerpo, 'Cuenta «' + cuerpo.nombre + '» agregada.');
				salirEdicionCuenta();
			}
		} catch (err) { /* aviso en pantalla */ }
	});

	$('altaCuentaCancelar').addEventListener('click', salirEdicionCuenta);

	// ---------- navegador de mes ----------

	function renderMesNav() {
		var partes = vistaMes.split('-');
		var mes = Number(partes[1]);
		var anio = Number(partes[0]);
		$('mesVisto').textContent = MESES_LARGOS[mes - 1] + ' ' + anio;
		var esHoy = vistaMes === mesIso(estado.anio, estado.mes);
		$('mesHoy').hidden = esHoy;
		$('presuMes').hidden = esHoy;
		if (!esHoy) $('presuMes').textContent = 'MES VISTO: ' + MESES[mes - 1] + ' ' + anio;
	}

	$('mesPrev').addEventListener('click', function () { cambiarMes(-1); });
	$('mesNext').addEventListener('click', function () { cambiarMes(1); });

	function cambiarMes(delta) {
		var partes = vistaMes.split('-');
		var total = Number(partes[0]) * 12 + Number(partes[1]) - 1 + delta;
		vistaMes = mesIso(Math.floor(total / 12), (total % 12 + 1));
		renderMesNav();
		renderLibro();
		renderPresu();
	}

	$('mesHoy').addEventListener('click', function () {
		vistaMes = mesIso(estado.anio, estado.mes);
		renderMesNav();
		renderLibro();
		renderPresu();
	});

	// ---------- libro contable (mes visto) ----------

	function comparar(a, b) {
		var cmp;
		if (orden.clave === 'valor') {
			cmp = a.valor - b.valor;
		} else {
			cmp = String(a[orden.clave]).localeCompare(String(b[orden.clave]), 'es');
		}
		return cmp * orden.dir;
	}

	function renderLibro(nuevosIds) {
		nuevosIds = nuevosIds || {};
		var cuerpo = $('libroCuerpo');
		cuerpo.innerHTML = '';
		var moneda = $('fMoneda').value;
		var categoria = $('fCategoria').value;
		var concepto = $('fConcepto').value.trim().toLowerCase();

		var filas = [];
		for (var i = 0; i < estado.movimientos.length; i++) {
			var m = estado.movimientos[i];
			if (!esMesVisto(m.fecha)) continue;
			var ok = (!moneda || m.moneda === moneda) &&
				(!categoria || m.categoria === categoria) &&
				(!concepto || m.concepto.toLowerCase().includes(concepto));
			if (ok) filas.push(m);
		}
		filas.sort(comparar);

		for (var j = 0; j < filas.length; j++) {
			cuerpo.appendChild(filaLibro(filas[j], nuevosIds[filas[j].id]));
		}

		var sinAsientos = estado.movimientos.length === 0;
		$('libroVacioTexto').textContent = sinAsientos
			? 'EL LIBRO ESTÁ VACÍO · ANOTA TU PRIMER ASIENTO ARRIBA'
			: 'SIN ASIENTOS EN ESTE MES · NAVEGA OTRO MES O ANOTA ARRIBA';
		$('libroVacio').hidden = filas.length !== 0;
		$('libroConteo').textContent = filas.length + (filas.length === 1 ? ' ASIENTO' : ' ASIENTOS');
	}

	function filaLibro(m, nuevo) {
		var anul = !!anulados[m.id];
		var tr = el('tr', 'movimiento' + (anul ? ' fila-anulada' : '') + (nuevo ? ' fila-nueva' : ''));
		var futuro = esFuturo(m.fecha);
		if (futuro) tr.classList.add('fila-futura');

		tr.appendChild(celda('celda-fecha', fechaCorta(m.fecha) + (futuro ? ' · PRÓX.' : '')));
		tr.appendChild(celda('celda-cat', m.categoria));
		tr.appendChild(celda('celda-cuenta', m.cuenta));

		var tdC = el('td', 'celda-concepto');
		if (m.rec_id) {
			tdC.appendChild(el('span', 'sello-demo-h chip-rec', 'RECURRENTE'));
		}
		tdC.appendChild(document.createTextNode(m.concepto));
		if (nuevo && !reduceMotion && !anul) {
			var marca = el('span', 'estampa', 'ANOTADO');
			marca.setAttribute('aria-hidden', 'true');
			tdC.appendChild(marca);
		}
		tr.appendChild(tdC);

		tr.appendChild(celda('celda-monto ' + (m.valor < 0 ? 'egreso' : 'ingreso'), montoTxt(m.valor, m.moneda)));

		var tdA = el('td', 'fila-acciones');
		if (anul) {
			tdA.appendChild(boton('DESHACER', function () { return function () { deshacer(m.id); }; }, null, 'btn-deshacer'));
		} else {
			tdA.appendChild(boton('EDITAR', function (mov) { return function () { entrarEdicionAsiento(mov); }; }, m));
			tdA.appendChild(boton('BORRAR', function (mov) { return function () { borrarAsiento(mov); }; }, m, 'btn-mini-peligro'));
			if (m.rec_id) {
				var bRec = el('button', 'btn btn-mini btn-mini-peligro', 'BORRAR REC.');
				bRec.type = 'button';
				bRec.title = 'Corta la recurrencia: borra los asientos futuros';
				bRec.addEventListener('click', function () { borrarRecurrencia(m.rec_id); });
				tdA.appendChild(bRec);
			}
		}
		tr.appendChild(tdA);
		return tr;
	}

	function celda(clase, texto) {
		return el('td', clase, texto);
	}

	function borrarAsiento(m) {
		var detalle = '«' + m.concepto + '» (' + fechaCorta(m.fecha) + ' · ' + montoTxt(m.valor, m.moneda) + ')';
		if (!window.confirm('¿Borrar el asiento ' + detalle + '?')) return;
		anulados[m.id] = true;
		timersUndo[m.id] = setTimeout(function () { caer(m.id); }, 4000);
		renderLibro();
	}

	function deshacer(id) {
		clearTimeout(timersUndo[id]);
		delete timersUndo[id];
		delete anulados[id];
		renderLibro();
	}

	async function caer(id) {
		clearTimeout(timersUndo[id]);
		delete timersUndo[id];
		delete anulados[id];
		try {
			await mutar('/api/movimientos/' + id, 'DELETE', undefined, 'Asiento borrado.');
		} catch (e) { /* aviso en pantalla */ }
	}

	async function borrarRecurrencia(recId) {
		var futuros = 0;
		var nombre = 'recurrencia';
		for (var i = 0; i < estado.movimientos.length; i++) {
			var m = estado.movimientos[i];
			if (m.rec_id === recId) {
				if (esFuturo(m.fecha)) futuros++;
				nombre = m.concepto;
			}
		}
		if (!window.confirm('¿Borrar la recurrencia «' + nombre + '»? Se borran ' + futuros + ' asiento(s) futuro(s); los ya cargados se conservan.')) return;
		try {
			await mutar('/api/recurrencias/' + recId, 'DELETE', undefined, 'Recurrencia cortada; los asientos ya cargados quedan.');
		} catch (e) { /* aviso en pantalla */ }
	}

	// ---------- presupuestos / categorías (mes visto) ----------

	function usadoDeCategoria(cat) {
		var usado = 0;
		for (var i = 0; i < estado.movimientos.length; i++) {
			var m = estado.movimientos[i];
			if (!activo(m) || !esMesVisto(m.fecha)) continue;
			if (m.categoria === cat.nombre && m.moneda === cat.moneda && m.valor < 0) {
				usado += -m.valor;
			}
		}
		return usado;
	}

	function renderPresu() {
		var lista = $('presuLista');
		lista.innerHTML = '';
		for (var i = 0; i < estado.categorias.length; i++) {
			var cat = estado.categorias[i];
			var usado = usadoDeCategoria(cat);
			var li = el('li', 'presu-item');
			if (cat.limite === null) {
				li.classList.add('presu-sin-limite');
				li.appendChild(el('span', 'presu-nombre', cat.nombre));
				li.appendChild(el('div', 'presu-barra presu-sin-barra'));
				li.appendChild(el('span', 'presu-lectura datos', fmt(usado, cat.moneda) + ' ' + cat.moneda + ' este mes · SIN LÍMITE'));
			} else {
				var pct = Math.round((usado / cat.limite) * 100);
				var excedido = pct > 100;
				if (excedido) li.classList.add('presu-excedido');
				li.appendChild(el('span', 'presu-nombre', cat.nombre));
				var barra = el('div', 'presu-barra');
				barra.setAttribute('role', 'progressbar');
				barra.setAttribute('aria-valuenow', String(Math.min(pct, 100)));
				barra.setAttribute('aria-valuemin', '0');
				barra.setAttribute('aria-valuemax', '100');
				barra.setAttribute('aria-label', 'Ejecución de ' + cat.nombre);
				barra.style.setProperty('--pct', Math.min(pct, 100) + '%');
				li.appendChild(barra);
				var lectura = el('span', 'presu-lectura datos');
				lectura.textContent = fmt(usado, cat.moneda) + ' / ' + fmt(cat.limite, cat.moneda) + ' ' + cat.moneda + ' ';
				lectura.appendChild(el('span', 'presu-pct', pct + '%'));
				if (excedido) lectura.appendChild(el('span', 'etiqueta-excedido', 'EXCEDIDO'));
				lectura.appendChild(document.createTextNode(' · ANUAL ' + fmt(cat.limite * 12, cat.moneda)));
				li.appendChild(lectura);
			}
			var acciones = el('div', 'presu-acciones');
			acciones.appendChild(boton('EDITAR', function (c2) { return function () { entrarEdicionCat(c2); }; }, cat));
			acciones.appendChild(boton('BORRAR', function (c3) { return function () { borrarCat(c3); }; }, cat, 'btn-mini-peligro'));
			li.appendChild(acciones);
			lista.appendChild(li);
		}
		$('estadoPresu').hidden = estado.categorias.length !== 0;
		renderMesNav();
	}

	async function borrarCat(cat) {
		var conAsientos = 0;
		for (var i = 0; i < estado.movimientos.length; i++) {
			if (estado.movimientos[i].categoria === cat.nombre) conAsientos++;
		}
		var msg = '¿Borrar la categoría «' + cat.nombre + '»?';
		if (conAsientos) msg += ' Sus ' + conAsientos + ' asiento(s) conservan la categoría como texto.';
		if (!window.confirm(msg)) return;
		try {
			await mutar('/api/categorias/' + cat.id, 'DELETE', undefined, 'Categoría «' + cat.nombre + '» borrada.');
		} catch (e) { /* aviso en pantalla */ }
	}

	function entrarEdicionCat(cat) {
		editarCatId = cat.id;
		$('altaCatTitulo').textContent = 'EDITAR CATEGORÍA';
		$('altaCatNombre').value = cat.nombre;
		$('altaCatLimite').value = cat.limite === null ? '' : cat.limite;
		$('altaCatMoneda').value = cat.moneda;
		$('altaCatBtn').textContent = 'GUARDAR';
		$('altaCatCancelar').hidden = false;
		$('altaCatNombre').focus();
	}

	function salirEdicionCat() {
		editarCatId = null;
		$('altaCatTitulo').textContent = 'AGREGAR CATEGORÍA';
		$('altaCatForm').reset();
		$('altaCatBtn').textContent = 'AGREGAR';
		$('altaCatCancelar').hidden = true;
	}

	$('altaCatForm').addEventListener('submit', async function (e) {
		e.preventDefault();
		var limiteTxt = ($('altaCatLimite').value || '').replace(',', '.');
		var cuerpo = {
			nombre: $('altaCatNombre').value.trim(),
			limite: limiteTxt === '' ? null : parseFloat(limiteTxt),
			moneda: $('altaCatMoneda').value,
		};
		try {
			if (editarCatId !== null) {
				await mutar('/api/categorias/' + editarCatId, 'PATCH', cuerpo, 'Categoría «' + cuerpo.nombre + '» guardada.');
				salirEdicionCat();
			} else {
				await mutar('/api/categorias', 'POST', cuerpo, 'Categoría «' + cuerpo.nombre + '» agregada.');
				salirEdicionCat();
			}
		} catch (err) { /* aviso en pantalla */ }
	});

	$('altaCatCancelar').addEventListener('click', salirEdicionCat);

	// ---------- carteles (12 meses del año, derivados de asientos reales) ----------

	function renderCarteles() {
		var grilla = $('cartelesGrilla');
		grilla.innerHTML = '';
		var Wc = Math.max(320, Math.round((grilla.clientWidth - 22) / 2));
		['ARS', 'USD'].forEach(function (moneda) {
			var serie = serieDelAno(moneda);
			var ingTotal = 0;
			var gasTotal = 0;
			for (var m = 0; m < 12; m++) {
				if (serie[m]) {
					ingTotal += serie[m][0];
					gasTotal += serie[m][1];
				}
			}
			var articulo = el('article', 'cartel');
			var titulos = el('div', 'cartel-titulos');
			var head = el('span', 'cartel-head');
			head.appendChild(el('small', null, 'AÑO ' + estado.anio + ' · ' + moneda));
			head.appendChild(document.createTextNode('ING ' + fmt(ingTotal, moneda) + ' · GAS ' + fmt(gasTotal, moneda)));
			titulos.appendChild(head);
			articulo.appendChild(titulos);
			articulo.appendChild(graficoAnual(serie, 'grafico', 'Ingresos y egresos mensuales en ' + moneda + ', año ' + estado.anio, Wc));
			var leyenda = el('div', 'leyenda rotulo');
			var l1 = el('span');
			l1.appendChild(el('i', 'ley-inge'));
			l1.appendChild(document.createTextNode('INGRESOS'));
			var l2 = el('span');
			l2.appendChild(el('i', 'ley-egre'));
			l2.appendChild(document.createTextNode('EGRESOS'));
			leyenda.appendChild(l1);
			leyenda.appendChild(l2);
			if (ingTotal === 0 && gasTotal === 0) {
				leyenda.appendChild(el('span', 'leyenda-nota', 'SIN DATOS AÚN · CARGA ASIENTOS Y VOLVÉ'));
			}
			articulo.appendChild(leyenda);
			grilla.appendChild(articulo);
		});
	}

	// ---------- captura: alta o modo edición, con recurrencia ----------

	function sincronizarSelects() {
		var cuentaSel = $('capCuenta');
		var actual = cuentaSel.value;
		cuentaSel.innerHTML = '';
		for (var i = 0; i < estado.cuentas.length; i++) {
			var o = el('option', null, estado.cuentas[i].nombre + ' · ' + estado.cuentas[i].moneda);
			o.value = String(estado.cuentas[i].id);
			cuentaSel.appendChild(o);
		}
		if (actual) cuentaSel.value = actual;

		var catSel = $('capCategoria');
		var fSel = $('fCategoria');
		var c2 = catSel.value;
		var f2 = fSel.value;
		catSel.innerHTML = '';
		fSel.innerHTML = '';
		var vaciaF = el('option', null, 'TODAS CATEGORÍAS');
		vaciaF.value = '';
		fSel.appendChild(vaciaF);
		for (var j = 0; j < estado.categorias.length; j++) {
			var nombre = estado.categorias[j].nombre;
			var o2 = el('option', null, nombre.toUpperCase());
			o2.value = nombre;
			catSel.appendChild(o2);
			var o3 = el('option', null, nombre.toUpperCase());
			o3.value = nombre;
			fSel.appendChild(o3);
		}
		if (c2) catSel.value = c2;
		if (f2) fSel.value = f2;
	}

	function renderTodo(nuevosIds) {
		renderMando();
		renderCuentas();
		renderPresu();
		renderLibro(nuevosIds);
		renderCarteles();
		renderMesNav();
		sincronizarSelects();
	}

	$('capturaForm').addEventListener('submit', async function (e) {
		e.preventDefault();

		var fecha = $('capFecha').value;
		var monto = parseFloat(($('capMonto').value || '').replace(',', '.'));
		var concepto = $('capConcepto').value.trim();
		var cuentaId = parseInt($('capCuenta').value, 10);
		var catId = parseInt($('capCategoria').value, 10);
		var moneda = $('capMoneda').value;
		var egreso = document.querySelector('input[name="capTipo"]:checked').value === 'egreso';
		var rec = $('capRec').checked;
		var cuotasTxt = $('capCuotas').value;
		var cuotas = cuotasTxt === '' ? null : parseInt(cuotasTxt, 10);

		if (!fecha || !concepto || !Number.isFinite(monto) || monto <= 0) {
			mostrarEstado('Falta fecha, concepto o un monto mayor que cero. Corrige y anota de nuevo.', 'aviso-error');
			return;
		}
		if (!Number.isFinite(cuentaId)) {
			mostrarEstado('Primero agrega una cuenta en la sección CUENTAS.', 'aviso-error');
			return;
		}

		var cuerpo = {
			fecha: fecha,
			cuenta_id: cuentaId,
			categoria_id: Number.isFinite(catId) ? catId : null,
			concepto: concepto,
			monto: monto,
			es_ingreso: !egreso,
			moneda: moneda,
		};
		if (rec) cuerpo.rec = { cuotas: cuotas };

		try {
			var r;
			if (editarMvId !== null) {
				cuerpo.alcance = alcanceElegido();
				r = await pedir('/api/movimientos/' + editarMvId, {
					method: 'PATCH',
					headers: { 'Content-Type': 'application/json' },
					body: JSON.stringify(cuerpo),
				});
			} else {
				r = await pedir('/api/movimientos', {
					method: 'POST',
					headers: { 'Content-Type': 'application/json' },
					body: JSON.stringify(cuerpo),
				});
			}
			if (!r.ok) {
				mostrarEstado((await r.text()) || 'No se pudo registrar', 'aviso-error');
				return;
			}
			var data = await r.json();
			var nuevos = {};
			nuevos[data.id] = true;
			await cargarEstado();
			renderTodo(nuevos);

			if (editarMvId !== null) {
				editarMvId = null;
				capturaModoAlta();
				mostrarEstado(
					data.futuros > 0
						? 'Asiento guardado + ' + data.futuros + ' asiento(s) siguiente(s) actualizado(s).'
						: 'Asiento guardado.',
					'aviso-exito'
				);
			} else {
				var msg = data.futuros > 0
					? 'Asiento anotado + ' + data.futuros + ' asiento(s) futuro(s) ' + (cuotas === null ? 'de la recurrencia' : 'de las ' + cuotas + ' cuotas') + '.'
					: 'Asiento anotado en el libro.';
				capturaModoAlta();
				mostrarEstado(msg, 'aviso-exito');
				$('capMonto').focus();
			}
		} catch (err) {
			mostrarEstado('Error de conexión con el servidor. Intenta de nuevo.', 'aviso-error');
		}
	});

	function capturaModoAlta() {
		$('capturaTitulo').textContent = 'ANOTAR EN EL LIBRO';
		$('capturaBtn').textContent = 'REGISTRAR';
		$('capturaCancelar').hidden = true;
		$('capturaForm').classList.remove('captura-edit');
		$('capturaForm').reset();
		editarEsDeRec = false;
		$('capAlcance').classList.remove('cap-alcance-on');
		$('capRec').disabled = false;
		$('capCuotas').disabled = false;
		$('capFecha').value = estado.hoy;
		actualizarRecFila();
	}

	function entrarEdicionAsiento(m) {
		editarMvId = m.id;
		$('capturaTitulo').textContent = 'EDITAR ASIENTO';
		$('capFecha').value = m.fecha;
		$('capMonto').value = String(Math.abs(m.valor));
		$('capMoneda').value = m.moneda;
		document.querySelector('input[name="capTipo"][value="' + (m.valor < 0 ? 'egreso' : 'ingreso') + '"]').checked = true;
		sincronizarSelects();
		$('capCuenta').value = String(m.cuenta_id);
		$('capCategoria').value = m.categoria_id === null ? '' : m.categoria;
		$('capConcepto').value = m.concepto;
		var rec = null;
		for (var i = 0; i < estado.recurrencias.length; i++) {
			if (estado.recurrencias[i].id === m.rec_id) rec = estado.recurrencias[i];
		}
		editarEsDeRec = !!m.rec_id;
		$('capAlcance').classList.toggle('cap-alcance-on', editarEsDeRec);
		document.querySelector('input[name="capAlcance"][value="unica"]').checked = true;
		$('capRec').checked = !!rec;
		$('capCuotas').value = rec && rec.cuotas !== null ? String(rec.cuotas) : '';
		actualizarRecFila();
		sincronizarAlcance();
		$('capturaBtn').textContent = 'GUARDAR';
		$('capturaCancelar').hidden = false;
		$('capturaForm').classList.add('captura-edit');
		$('capMonto').focus();
	}

	$('capturaCancelar').addEventListener('click', function () {
		editarMvId = null;
		capturaModoAlta();
	});

	$('capRec').addEventListener('change', actualizarRecFila);

	function actualizarRecFila() {
		$('capRec').closest('.cap-rec').classList.toggle('cap-rec-on', $('capRec').checked);
	}

	// ---------- alcance de la edición de una fila recurrente ----------

	function alcanceElegido() {
		var r = document.querySelector('input[name="capAlcance"]:checked');
		return r ? r.value : 'unica';
	}

	function sincronizarAlcance() {
		// con "solo este asiento" la serie no se toca: se congelan sus controles
		var congelado = editarEsDeRec && alcanceElegido() === 'unica';
		$('capRec').disabled = congelado;
		$('capCuotas').disabled = congelado;
	}

	document.querySelectorAll('input[name="capAlcance"]').forEach(function (r) {
		r.addEventListener('change', sincronizarAlcance);
	});

	// ---------- orden ----------

	$('libroTabla').querySelectorAll('th[data-ordena] button').forEach(function (btn) {
		btn.addEventListener('click', function () {
			var th = btn.closest('th');
			var clave = th.dataset.ordena;
			if (orden.clave === clave) {
				orden.dir = -orden.dir;
			} else {
				orden.clave = clave;
				orden.dir = -1;
			}
			var ths = $('libroTabla').querySelectorAll('th[data-ordena]');
			for (var i = 0; i < ths.length; i++) {
				ths[i].setAttribute('aria-sort', ths[i].dataset.ordena === orden.clave ? (orden.dir === 1 ? 'ascending' : 'descending') : 'none');
			}
			renderLibro();
		});
	});

	// ---------- filtros ----------

	$('fMoneda').addEventListener('change', function () { renderLibro(); });
	$('fCategoria').addEventListener('change', function () { renderLibro(); });
	$('fConcepto').addEventListener('input', function () { renderLibro(); });
	$('fLimpiar').addEventListener('click', function () {
		$('fMoneda').value = '';
		$('fCategoria').value = '';
		$('fConcepto').value = '';
		renderLibro();
	});

	// ---------- accesos de estados vacíos ----------

	$('estadoCuentasBtn').addEventListener('click', function () { $('altaNombre').focus(); });
	$('estadoPresuBtn').addEventListener('click', function () { $('altaCatNombre').focus(); });

	// ---------- charts responsivos ----------

	var resizePendiente = false;
	window.addEventListener('resize', function () {
		if (resizePendiente) return;
		resizePendiente = true;
		requestAnimationFrame(function () {
			resizePendiente = false;
			if (estado) {
				renderMando();
				renderCarteles();
			}
		});
	});

	// ---------- logout ----------

	$('logoutBtn').addEventListener('click', async function () {
		await fetch('/api/logout', { method: 'POST' });
		window.location.href = '/login';
	});

	// ---------- arranque ----------

	cargarEstado()
		.then(function () { renderTodo(); })
		.catch(function (e) {
			if (String(e.message) !== 'sesión expirada') {
				var marco = document.querySelector('main');
				var caja = el('div', 'estado-vacio');
				caja.appendChild(el('span', 'rotulo', 'EL SERVIDOR NO CONTESTA EL ESTADO · RECARGA LA PÁGINA'));
				if (marco) marco.appendChild(caja);
			}
		});
})();
