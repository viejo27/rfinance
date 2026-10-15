// RFinance — home: estado en memoria + render client-side.
// Nada se persiste: todo vive en esta pestaña y se borra al recargar.
// Los datos semilla son de ejemplo, editables y borrables con el CRUD.
// El Sello (alta), el corte duro steps(2) y los materiales del mundo
// quedan a cargo de style.css / dashboard.css.

(function () {
	'use strict';

	// ---------- utilidades ----------

	var nfARS = new Intl.NumberFormat('es-AR', { maximumFractionDigits: 0 });
	var nfUSD = new Intl.NumberFormat('es-AR', { minimumFractionDigits: 2, maximumFractionDigits: 2 });

	var MESES = ['ENE', 'FEB', 'MAR', 'ABR', 'MAY', 'JUN', 'JUL', 'AGO', 'SEP', 'OCT', 'NOV', 'DIC'];
	var reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
	var HOY_ISO = document.body.dataset.hoy || new Date().toISOString().slice(0, 10);
	var ANO = Number(HOY_ISO.slice(0, 4));
	var MES_HOY = Number(HOY_ISO.slice(5, 7));
	var DIA_HOY = Number(HOY_ISO.slice(8, 10));

	function fmt(valor, moneda) {
		return (moneda === 'USD' ? nfUSD : nfARS).format(valor);
	}

	function montoTxt(valor, moneda) {
		return (valor < 0 ? '−' : '') + fmt(Math.abs(valor), moneda) + ' ' + moneda;
	}

	function fechaCorta(iso) {
		return iso.slice(8, 10) + ' ' + MESES[Number(iso.slice(5, 7)) - 1];
	}

	function ultimoDia(mes) {
		return new Date(ANO, mes, 0).getDate();
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

	// ---------- estado semilla (datos de ejemplo, editables y borrables) ----------

	var estado = {
		seq: 100,
		cuentas: [
			{ id: 1, nombre: 'Efectivo', moneda: 'ARS', saldoInicial: 84520 },
			{ id: 2, nombre: 'Banco Nación', moneda: 'ARS', saldoInicial: 1632470 },
			{ id: 3, nombre: 'Visa ARS', moneda: 'ARS', saldoInicial: -78215 },
			{ id: 4, nombre: 'Reserva USD', moneda: 'USD', saldoInicial: 4650 },
		],
		categorias: [
			{ id: 10, nombre: 'Alquiler', limite: 320000, moneda: 'ARS' },
			{ id: 11, nombre: 'Alimentación', limite: 120000, moneda: 'ARS' },
			{ id: 12, nombre: 'Transporte', limite: 60000, moneda: 'ARS' },
			{ id: 13, nombre: 'Servicios', limite: 90000, moneda: 'ARS' },
			{ id: 14, nombre: 'Suscripciones', limite: 30, moneda: 'USD' },
			{ id: 15, nombre: 'Salud', limite: null, moneda: 'ARS' },
			{ id: 16, nombre: 'Ocio', limite: null, moneda: 'ARS' },
			{ id: 17, nombre: 'Ingresos', limite: null, moneda: 'ARS' },
		],
		movimientos: [
			mv(1, '2026-10-12', 'Alquiler', 'Banco Nación', 'Alquiler mes de octubre', -320000, 'ARS', 1),
			mv(2, '2026-10-12', 'Alimentación', 'Visa ARS', 'Supermercado La Anónima', -46800, 'ARS', null),
			mv(3, '2026-10-11', 'Transporte', 'Efectivo', 'Nafta Shell, tanque lleno', -22400, 'ARS', null),
			mv(4, '2026-10-11', 'Transporte', 'Efectivo', 'Tarifa de subte (20 viajes)', -3150, 'ARS', null),
			mv(5, '2026-10-10', 'Suscripciones', 'Reserva USD', 'Netflix, plan familiar', -14.99, 'USD', null),
			mv(6, '2026-10-10', 'Salud', 'Banco Nación', 'Prepaga Swiss Medical', -38900, 'ARS', null),
			mv(7, '2026-10-09', 'Alimentación', 'Efectivo', 'Verdulería Doña Rosa', -8750, 'ARS', null),
			mv(8, '2026-10-08', 'Ingresos', 'Banco Nación', 'Sueldo de octubre', 950000, 'ARS', null),
			mv(9, '2026-10-07', 'Ingresos', 'Reserva USD', 'Honorario freelance (Denver)', 400, 'USD', null),
			mv(10, '2026-10-06', 'Servicios', 'Banco Nación', 'Luz + ABL + gas', -27500, 'ARS', null),
			mv(11, '2026-10-05', 'Ocio', 'Visa ARS', 'Cine y panchos, domingo', -19300, 'ARS', null),
			mv(12, '2026-10-04', 'Suscripciones', 'Visa ARS', 'Spotify familiar', -2850, 'ARS', null),
			mv(13, '2026-10-03', 'Ingresos', 'Efectivo', 'Reparación de PC, contado', 45000, 'ARS', null),
			mv(14, '2026-10-02', 'Alimentación', 'Efectivo', 'Almuerzo en el trabajo', -6900, 'ARS', null),
			mv(15, '2026-10-12', 'Ocio', 'Visa ARS', 'Notebook en 12 cuotas', -55000, 'ARS', 2),
		],
		recurrencias: {
			1: { cuotas: null, concepto: 'Alquiler' },
			2: { cuotas: 12, concepto: 'Notebook' },
		},
	};

	function mv(id, fecha, categoria, cuenta, concepto, valor, moneda, recId) {
		return { id: id, fecha: fecha, categoria: categoria, cuenta: cuenta, concepto: concepto, valor: valor, moneda: moneda, recId: recId };
	}

	// materializa los meses futuros del año calendario para cada recurrencia
	function materializarRecurrencias() {
		var extra = [];
		Object.keys(estado.recurrencias).forEach(function (rid) {
			extra = extra.concat(generarFuturos(Number(rid), 2, estado.recurrencias[rid].cuotas, null));
		});
		estado.movimientos = estado.movimientos.concat(extra);
	}

	function generarFuturos(recId, cuotaDesde, cuotas, origenExplicito) {
		var origen = origenExplicito || primeraFilaRec(recId);
		if (!origen) return [];
		var futuros = [];
		var cuota = cuotaDesde;
		var base = origen.concepto.replace(/ en \d+ cuotas?$/i, '').replace(/,? mes de \w+$/i, '');
		var dia = Math.min(Number(origen.fecha.slice(8, 10)), 28);
		for (var mes = MES_HOY + 1; mes <= 12; mes++) {
			if (cuotas !== null && cuota > cuotas) break;
			dia = Math.min(dia, ultimoDia(mes));
			var iso = ANO + '-' + String(mes).padStart(2, '0') + '-' + String(dia).padStart(2, '0');
			var texto = cuotas !== null
				? 'Cuota ' + cuota + '/' + cuotas + ', ' + base
				: base + ', mes ' + MESES[mes - 1].toLowerCase();
			futuros.push(mv(estado.seq++, iso, origen.categoria, origen.cuenta, texto, origen.valor, origen.moneda, recId));
			cuota++;
		}
		return futuros;
	}

	function primeraFilaRec(recId) {
		for (var i = 0; i < estado.movimientos.length; i++) {
			if (estado.movimientos[i].recId === recId) return estado.movimientos[i];
		}
		return null;
	}

	materializarRecurrencias();

	// ---------- derivaciones ----------

	// anulados: ids en ventana DESHACER — quedan pintados tachados,
	// excluidos de saldos, totales y presupuestos hasta caer.
	var anulados = {};
	var timersUndo = {};

	function activo(m) {
		return !anulados[m.id];
	}

	function saldoDeCuenta(c) {
		var s = c.saldoInicial;
		for (var i = 0; i < estado.movimientos.length; i++) {
			var m = estado.movimientos[i];
			if (activo(m) && m.cuenta === c.nombre && m.moneda === c.moneda) s += m.valor;
		}
		return s;
	}

	function usadoDeCategoria(cat) {
		var usado = 0;
		for (var i = 0; i < estado.movimientos.length; i++) {
			var m = estado.movimientos[i];
			var mes = Number(m.fecha.slice(5, 7));
			if (activo(m) && m.categoria === cat.nombre && m.moneda === cat.moneda && m.valor < 0 && mes === MES_HOY) {
				usado += -m.valor;
			}
		}
		return usado;
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
			porMoneda[c.moneda] += saldoDeCuenta(c);
			nombresPorMoneda[c.moneda].push(c.nombre);
		}
		$('totalArs').textContent = fmt(porMoneda.ARS, 'ARS');
		$('totalUsd').textContent = fmt(porMoneda.USD, 'USD');
		$('totalArs').closest('.mando-total').classList.toggle('total-neg', porMoneda.ARS < 0);
		$('totalUsd').closest('.mando-total').classList.toggle('total-neg', porMoneda.USD < 0);
		$('subArs').textContent = nombresPorMoneda.ARS.join(' · ') || 'sin cuentas ARS';
		$('subUsd').textContent = nombresPorMoneda.USD.join(' · ') || 'sin cuentas USD';

		var serie = SERIES_DEMO.ARS;
		var maximo = 0;
		var netoAnual = 0;
		for (var m = 0; m < 12; m++) {
			if (serie[m]) {
				maximo = Math.max(maximo, serie[m][0], serie[m][1]);
				netoAnual += serie[m][0] - serie[m][1];
			}
		}
		var caja = $('mandoHisto');
		caja.innerHTML = '';
		caja.appendChild(graficoAño(serie, 'ARS', 'histo', 'Ingresos y egresos mensuales en pesos, año ' + ANO));
		$('netoAnual').textContent = '+ ' + fmt(netoAnual, 'ARS') + ' (serie de ejemplo)';
	}

	function graficoAño(serie, moneda, claseCss, etiquetaAria) {
		var maximo = 0;
		for (var m = 0; m < 12; m++) {
			if (serie[m]) maximo = Math.max(maximo, serie[m][0], serie[m][1]);
		}
		var g = svg('svg', { class: claseCss, viewBox: '0 0 660 240', role: 'img', 'aria-label': etiquetaAria });
		for (var m2 = 0; m2 < 12; m2++) {
			var x = 8 + m2 * 54;
			var par = serie[m2];
			if (par) {
				barra(g, x, par[0], maximo, 'var(--ocre-barra)', MESES[m2] + ' ingresos');
				barra(g, x + 18, par[1], maximo, 'var(--rojo)', MESES[m2] + ' egresos');
			} else {
				g.appendChild(svg('rect', { x: x + 2, y: 196, width: 12, height: 2, fill: 'var(--concreto)' }));
				g.appendChild(svg('rect', { x: x + 20, y: 196, width: 12, height: 2, fill: 'var(--concreto)' }));
			}
			g.appendChild(svg('text', { x: x + 16, y: 226, 'text-anchor': 'middle' })).textContent = MESES[m2];
		}
		g.appendChild(svg('line', { class: 'eje', x1: 4, y1: 208, x2: 656, y2: 208 }));
		return g;
	}

	function barra(grafico, x, valor, maximo, color, titulo) {
		var h = Math.max(2, Math.round((valor / maximo) * 190));
		var rect = svg('rect', { x: x, y: 208 - h, width: 14, height: h, fill: color });
		var t = svg('title');
		t.textContent = titulo + ': ' + valor;
		rect.appendChild(t);
		grafico.appendChild(rect);
	}

	// series de ejemplo del año calendario ENE→DIC ([ingreso, egreso] por mes)
	var SERIES_DEMO = {
		ARS: [
			[610000, 520000], [640000, 551000], [720000, 598000], [745000, 610000],
			[780000, 640000], [952000, 701500], [868000, 559200], [1340000, 812300],
			[986000, 664800], [1390000, 978600], null, null,
		],
		USD: [
			[380, 290], [420, 305], [510, 364], [560, 390],
			[600, 420], [850, 510], [400, 380], [1240, 690],
			[780, 455], [1150, 742], null, null,
		],
	};

	// ---------- cuentas ----------

	var editarCuentaId = null;

	function renderCuentas() {
		var grilla = $('cuentasGrilla');
		grilla.innerHTML = '';
		for (var i = 0; i < estado.cuentas.length; i++) {
			var c = estado.cuentas[i];
			var saldo = saldoDeCuenta(c);
			var bloque = el('div', 'cuenta' + (c.moneda === 'USD' ? ' cuenta-malla' : ''));
			bloque.appendChild(el('span', 'rotulo', c.nombre));
			bloque.appendChild(el('span', 'cuenta-saldo' + (saldo < 0 ? ' cuenta-neg' : ''), fmt(saldo, c.moneda)));
			bloque.appendChild(el('span', 'cuenta-chip', c.moneda));
			var acciones = el('div', 'cuenta-acciones');
			acciones.appendChild(boton('EDITAR', function (cta) { return function () { entrarEdicionCuenta(cta.id); }; }, c));
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

	function borrarCuenta(c) {
		var n = 0;
		for (var i = 0; i < estado.movimientos.length; i++) if (estado.movimientos[i].cuenta === c.nombre) n++;
		var msg = '¿Borrar la cuenta «' + c.nombre + '»?';
		if (n) msg += ' Sus ' + n + ' asiento(s) del libro también se borran.';
		msg += ' (Demo: solo en esta pestaña.)';
		if (!window.confirm(msg)) return;
		estado.cuentas = estado.cuentas.filter(function (x) { return x.id !== c.id; });
		estado.movimientos = estado.movimientos.filter(function (m) { return m.cuenta !== c.nombre; });
		if (editarCuentaId === c.id) salirEdicionCuenta();
		renderTodo();
		mostrarEstado('Cuenta «' + c.nombre + '» borrada (demo, en memoria).', 'aviso-exito');
	}

	function entrarEdicionCuenta(id) {
		var c = null;
		for (var i = 0; i < estado.cuentas.length; i++) if (estado.cuentas[i].id === id) c = estado.cuentas[i];
		if (!c) return;
		editarCuentaId = id;
		$('altaTitulo').textContent = 'EDITAR CUENTA';
		$('altaNombre').value = c.nombre;
		$('altaNombre').dataset.previo = c.nombre;
		$('altaMoneda').value = c.moneda;
		$('altaSaldo').value = c.saldoInicial;
		$('altaCuentaBtn').textContent = 'GUARDAR';
		$('altaCuentaCancelar').hidden = false;
		$('altaNombre').focus();
	}

	function salirEdicionCuenta() {
		editarCuentaId = null;
		delete $('altaNombre').dataset.previo;
		$('altaTitulo').textContent = 'AGREGAR CUENTA';
		$('altaCuentaForm').reset();
		$('altaCuentaBtn').textContent = 'AGREGAR';
		$('altaCuentaCancelar').hidden = true;
	}

	$('altaCuentaForm').addEventListener('submit', function (e) {
		e.preventDefault();
		var nombre = $('altaNombre').value.trim();
		var moneda = $('altaMoneda').value;
		var saldo = parseFloat(($('altaSaldo').value || '0').replace(',', '.'));
		if (!nombre) return;
		if (editarCuentaId !== null) {
			var previo = $('altaNombre').dataset.previo;
			for (var i = 0; i < estado.cuentas.length; i++) if (estado.cuentas[i].id === editarCuentaId) {
				estado.cuentas[i].nombre = nombre;
				estado.cuentas[i].moneda = moneda;
				estado.cuentas[i].saldoInicial = saldo;
			}
			for (var j = 0; j < estado.movimientos.length; j++) if (estado.movimientos[j].cuenta === previo) estado.movimientos[j].cuenta = nombre;
			salirEdicionCuenta();
			renderTodo();
			mostrarEstado('Cuenta «' + nombre + '» guardada (demo, en memoria).', 'aviso-exito');
		} else {
			estado.cuentas.push({ id: estado.seq++, nombre: nombre, moneda: moneda, saldoInicial: saldo });
			salirEdicionCuenta();
			renderTodo();
			mostrarEstado('Cuenta «' + nombre + '» agregada (demo, en memoria).', 'aviso-exito');
		}
	});

	$('altaCuentaCancelar').addEventListener('click', function () {
		salirEdicionCuenta();
		renderCuentas();
	});

	// ---------- libro contable ----------

	var orden = { clave: 'fecha', dir: -1 };

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
		var filas = estado.movimientos.slice().sort(comparar);
		var visibles = 0;
		for (var i = 0; i < filas.length; i++) {
			var m = filas[i];
			var ok = (!moneda || m.moneda === moneda) &&
				(!categoria || m.categoria === categoria) &&
				(!concepto || m.concepto.toLowerCase().includes(concepto));
			if (!ok) continue;
			visibles++;
			cuerpo.appendChild(filaLibro(m, nuevosIds[m.id]));
		}
		$('libroVacio').hidden = visibles !== 0;
		$('libroConteo').textContent = visibles + (visibles === 1 ? ' ASIENTO' : ' ASIENTOS');
	}

	function filaLibro(m, nuevo) {
		var anul = !!anulados[m.id];
		var tr = el('tr', 'movimiento' + (anul ? ' fila-anulada' : '') + (nuevo ? ' fila-nueva' : ''));
		var futuro = m.fecha > HOY_ISO;
		if (futuro) tr.classList.add('fila-futura');

		tr.appendChild(celda('celda-fecha', fechaCorta(m.fecha) + (futuro ? ' · PRÓX.' : '')));
		tr.appendChild(celda('celda-cat', m.categoria));
		tr.appendChild(celda('celda-cuenta', m.cuenta));

		var tdC = el('td', 'celda-concepto');
		if (m.recId) {
			var chip = el('span', 'sello-demo-h chip-rec', 'RECURRENTE');
			tdC.appendChild(chip);
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
			tdA.appendChild(boton('EDITAR', function (mov) { return function () { entrarEdicionAsiento(mov.id); }; }, m));
			tdA.appendChild(boton('BORRAR', function (mov) { return function () { borrarAsiento(mov.id); }; }, m, 'btn-mini-peligro'));
			if (m.recId) {
				var bRec = el('button', 'btn btn-mini btn-mini-peligro', 'BORRAR REC.');
				bRec.type = 'button';
				bRec.title = 'Corta la recurrencia: borra los asientos futuros de este año';
				bRec.addEventListener('click', (function (mov) { return function () { borrarRecurrencia(mov.recId); }; })(m));
				tdA.appendChild(bRec);
			}
		}
		tr.appendChild(tdA);
		return tr;
	}

	function celda(clase, texto) {
		return el('td', clase, texto);
	}

	function borrarAsiento(id) {
		var m = null;
		for (var i = 0; i < estado.movimientos.length; i++) if (estado.movimientos[i].id === id) m = estado.movimientos[i];
		if (!m) return;
		var detalle = '«' + m.concepto + '» (' + fechaCorta(m.fecha) + ' · ' + montoTxt(m.valor, m.moneda) + ')';
		if (!window.confirm('¿Borrar el asiento ' + detalle + '? (Demo: solo en esta pestaña.)')) return;
		anulados[m.id] = true;
		timersUndo[m.id] = setTimeout(function () { caer(m.id); }, 4000);
		renderTodo();
	}

	function deshacer(id) {
		clearTimeout(timersUndo[id]);
		delete timersUndo[id];
		delete anulados[id];
		renderTodo();
	}

	function caer(id) {
		clearTimeout(timersUndo[id]);
		delete timersUndo[id];
		delete anulados[id];
		estado.movimientos = estado.movimientos.filter(function (x) { return x.id !== id; });
		renderTodo();
	}

	function borrarRecurrencia(recId) {
		var futuros = 0;
		for (var i = 0; i < estado.movimientos.length; i++) {
			var m = estado.movimientos[i];
			if (m.recId === recId && m.fecha > HOY_ISO) futuros++;
		}
		var origen = primeraFilaRec(recId);
		var nombre = origen ? origen.concepto : 'recurrencia';
		if (!window.confirm('¿Borrar la recurrencia «' + nombre + '»? Se borran ' + futuros + ' asiento(s) futuro(s) de este año; los ya cargados se conservan. (Demo.)')) return;
		estado.movimientos = estado.movimientos.filter(function (m2) {
			return !(m2.recId === recId && m2.fecha > HOY_ISO);
		});
		for (var j = 0; j < estado.movimientos.length; j++) {
			if (estado.movimientos[j].recId === recId) estado.movimientos[j].recId = null;
		}
		delete estado.recurrencias[recId];
		renderTodo();
		mostrarEstado('Recurrencia «' + nombre + '» cortada; los asientos ya cargados quedan.', 'aviso-exito');
	}

	// ---------- presupuestos / categorías ----------

	var editarCatId = null;

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
			acciones.appendChild(boton('EDITAR', function (c2) { return function () { entrarEdicionCat(c2.id); }; }, cat));
			acciones.appendChild(boton('BORRAR', function (c3) { return function () { borrarCat(c3); }; }, cat, 'btn-mini-peligro'));
			li.appendChild(acciones);
			lista.appendChild(li);
		}
		$('estadoPresu').hidden = estado.categorias.length !== 0;
		sincronizarSelects();
	}

	function borrarCat(cat) {
		var conAsientos = 0;
		for (var i = 0; i < estado.movimientos.length; i++) if (estado.movimientos[i].categoria === cat.nombre) conAsientos++;
		var msg = '¿Borrar la categoría «' + cat.nombre + '»?';
		if (conAsientos) msg += ' Sus ' + conAsientos + ' asiento(s) conservan la categoría como texto.';
		msg += ' (Demo.)';
		if (!window.confirm(msg)) return;
		estado.categorias = estado.categorias.filter(function (x) { return x.id !== cat.id; });
		if (editarCatId === cat.id) salirEdicionCat();
		renderTodo();
		mostrarEstado('Categoría «' + cat.nombre + '» borrada.', 'aviso-exito');
	}

	function entrarEdicionCat(id) {
		var cat = null;
		for (var i = 0; i < estado.categorias.length; i++) if (estado.categorias[i].id === id) cat = estado.categorias[i];
		if (!cat) return;
		editarCatId = id;
		var previo = cat.nombre;
		$('altaCatTitulo').textContent = 'EDITAR CATEGORÍA';
		$('altaCatNombre').value = cat.nombre;
		$('altaCatNombre').dataset.previo = previo;
		$('altaCatLimite').value = cat.limite === null ? '' : cat.limite;
		$('altaCatMoneda').value = cat.moneda;
		$('altaCatBtn').textContent = 'GUARDAR';
		$('altaCatCancelar').hidden = false;
		$('altaCatNombre').focus();
	}

	function salirEdicionCat() {
		editarCatId = null;
		delete $('altaCatNombre').dataset.previo;
		$('altaCatTitulo').textContent = 'AGREGAR CATEGORÍA';
		$('altaCatForm').reset();
		$('altaCatBtn').textContent = 'AGREGAR';
		$('altaCatCancelar').hidden = true;
	}

	$('altaCatForm').addEventListener('submit', function (e) {
		e.preventDefault();
		var nombre = $('altaCatNombre').value.trim();
		var limiteTxt = ($('altaCatLimite').value || '').replace(',', '.');
		var limite = limiteTxt === '' ? null : parseFloat(limiteTxt);
		var moneda = $('altaCatMoneda').value;
		if (!nombre) return;
		if (editarCatId !== null) {
			var previo = $('altaCatNombre').dataset.previo;
			for (var i = 0; i < estado.categorias.length; i++) if (estado.categorias[i].id === editarCatId) {
				estado.categorias[i].nombre = nombre;
				estado.categorias[i].limite = limite;
				estado.categorias[i].moneda = moneda;
			}
			for (var k = 0; k < estado.movimientos.length; k++) if (estado.movimientos[k].categoria === previo) estado.movimientos[k].categoria = nombre;
			salirEdicionCat();
			renderTodo();
			mostrarEstado('Categoría «' + nombre + '» guardada.', 'aviso-exito');
		} else {
			estado.categorias.push({ id: estado.seq++, nombre: nombre, limite: limite, moneda: moneda });
			salirEdicionCat();
			renderTodo();
			mostrarEstado('Categoría «' + nombre + '» agregada.', 'aviso-exito');
		}
	});

	$('altaCatCancelar').addEventListener('click', function () {
		salirEdicionCat();
		renderPresu();
	});

	// ---------- carteles (12 meses, año calendario ENE→DIC) ----------

	function renderCarteles() {
		var grilla = $('cartelesGrilla');
		grilla.innerHTML = '';
		['ARS', 'USD'].forEach(function (moneda) {
			var serie = SERIES_DEMO[moneda];
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
			head.appendChild(el('small', null, 'AÑO ' + ANO + ' · ' + moneda + ' · SERIE DE EJEMPLO'));
			head.appendChild(document.createTextNode('ING ' + fmt(ingTotal, moneda) + ' · GAS ' + fmt(gasTotal, moneda)));
			titulos.appendChild(head);
			articulo.appendChild(titulos);
			articulo.appendChild(graficoAño(serie, moneda, 'grafico', 'Ingresos y egresos mensuales en ' + moneda + ', año ' + ANO));
			var leyenda = el('div', 'leyenda rotulo');
			var l1 = el('span');
			l1.appendChild(el('i', 'ley-inge'));
			l1.appendChild(document.createTextNode('INGRESOS'));
			var l2 = el('span');
			l2.appendChild(el('i', 'ley-egre'));
			l2.appendChild(document.createTextNode('EGRESOS'));
			leyenda.appendChild(l1);
			leyenda.appendChild(l2);
			leyenda.appendChild(el('span', 'leyenda-nota', 'DATOS DE EJEMPLO'));
			articulo.appendChild(leyenda);
			grilla.appendChild(articulo);
		});
	}

	// ---------- captura: alta, modo edición, recurrencia ----------

	var editarMvId = null;

	function sincronizarSelects() {
		var cuentaSel = $('capCuenta');
		var actual = cuentaSel.value;
		cuentaSel.innerHTML = '';
		for (var i = 0; i < estado.cuentas.length; i++) {
			var o = el('option', null, estado.cuentas[i].nombre + ' · ' + estado.cuentas[i].moneda);
			o.value = estado.cuentas[i].nombre;
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
		sincronizarSelects();
	}

	// captura: alta o guardar edición
	$('capturaForm').addEventListener('submit', function (e) {
		e.preventDefault();

		var fecha = $('capFecha').value;
		var monto = parseFloat(($('capMonto').value || '').replace(',', '.'));
		var concepto = $('capConcepto').value.trim();
		var cuenta = $('capCuenta').value;
		var categoria = $('capCategoria').value;
		var moneda = $('capMoneda').value;
		var egreso = document.querySelector('input[name="capTipo"]:checked').value === 'egreso';
		var rec = $('capRec').checked;
		var cuotasTxt = $('capCuotas').value;
		var cuotas = cuotasTxt === '' ? null : parseInt(cuotasTxt, 10);

		if (!fecha || !concepto || !Number.isFinite(monto) || monto <= 0) {
			mostrarEstado('Falta fecha, concepto o un monto mayor que cero. Corrige y anota de nuevo.', 'aviso-error');
			return;
		}
		if (!cuenta) {
			mostrarEstado('Primero agrega una cuenta en la sección CUENTAS.', 'aviso-error');
			return;
		}

		var valor = egreso ? -monto : monto;

		if (editarMvId !== null) {
			for (var i = 0; i < estado.movimientos.length; i++) {
				if (estado.movimientos[i].id !== editarMvId) continue;
				var m = estado.movimientos[i];
				m.fecha = fecha;
				m.concepto = concepto;
				m.cuenta = cuenta;
				m.categoria = categoria;
				m.moneda = moneda;
				m.valor = valor;

				if (rec) {
					var recId = m.recId || estado.seq++;
					m.recId = recId;
					estado.recurrencias[recId] = { cuotas: cuotas, concepto: concepto };
					estado.movimientos = estado.movimientos.filter(function (x) {
						return !(x.recId === recId && x.fecha > HOY_ISO);
					});
					estado.movimientos = estado.movimientos.concat(generarFuturos(recId, 2, cuotas, m));
				} else if (m.recId) {
					estado.movimientos = estado.movimientos.filter(function (x) {
						return !(x.recId === m.recId && x.fecha > HOY_ISO);
					});
					delete estado.recurrencias[m.recId];
					m.recId = null;
				}
			}
			editarMvId = null;
			capturaModoAlta();
			renderTodo();
			mostrarEstado('Asiento guardado (demo, en memoria).', 'aviso-exito');
			return;
		}

		// alta con El Sello
		var id = estado.seq++;
		var nuevos = {};
		nuevos[id] = true;
		var nuevoMv = mv(id, fecha, categoria, cuenta, concepto, valor, moneda, null);
		var futuros = [];
		if (rec) {
			var recId2 = estado.seq++;
			nuevoMv.recId = recId2;
			estado.recurrencias[recId2] = { cuotas: cuotas, concepto: concepto };
		}
		estado.movimientos.push(nuevoMv);
		if (rec) {
			futuros = generarFuturos(recId2, 2, cuotas, nuevoMv);
			estado.movimientos = estado.movimientos.concat(futuros);
			for (var f = 0; f < futuros.length; f++) nuevos[futuros[f].id] = true;
		}

		renderTodo(nuevos);

		var msgFuturos = futuros.length
			? 'Asiento anotado + ' + futuros.length + ' asiento(s) futuro(s) ' + (cuotas === null ? 'de la recurrencia' : 'de las ' + cuotas + ' cuotas') + ' hasta fin de año' + (cuotas !== null && cuotas - 1 > futuros.length ? ' (el resto de las cuotas cae el año próximo; demo solo hasta diciembre)' : '') + '.'
			: 'Asiento anotado en el libro. Datos de ejemplo: no se guarda.';
		mostrarEstado(msgFuturos + ' (Demo, en memoria.)', 'aviso-exito');

		$('capMonto').value = '';
		$('capConcepto').value = '';
		$('capRec').checked = false;
		$('capCuotas').value = '';
		actualizarRecFila();
		$('capMonto').focus();
	});

	function capturaModoAlta() {
		$('capturaTitulo').textContent = 'ANOTAR EN EL LIBRO';
		$('capturaBtn').textContent = 'REGISTRAR';
		$('capturaCancelar').hidden = true;
		$('capturaForm').classList.remove('captura-edit');
		$('capturaForm').reset();
		$('capFecha').value = HOY_ISO;
		actualizarRecFila();
	}

	function entrarEdicionAsiento(id) {
		var m = null;
		for (var i = 0; i < estado.movimientos.length; i++) if (estado.movimientos[i].id === id) m = estado.movimientos[i];
		if (!m) return;
		editarMvId = id;
		$('capturaTitulo').textContent = 'EDITAR ASIENTO';
		$('capFecha').value = m.fecha;
		$('capMonto').value = String(Math.abs(m.valor));
		$('capMoneda').value = m.moneda;
		document.querySelector('input[name="capTipo"][value="' + (m.valor < 0 ? 'egreso' : 'ingreso') + '"]').checked = true;
		sincronizarSelects();
		$('capCuenta').value = m.cuenta;
		$('capCategoria').value = m.categoria;
		$('capConcepto').value = m.concepto;
		var rec = m.recId ? estado.recurrencias[m.recId] : null;
		$('capRec').checked = !!rec;
		$('capCuotas').value = rec && rec.cuotas !== null ? String(rec.cuotas) : '';
		actualizarRecFila();
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

	// ---------- logout ----------

	$('logoutBtn').addEventListener('click', async function () {
		await fetch('/api/logout', { method: 'POST' });
		window.location.href = '/login';
	});

	// arranque
	renderTodo();
})();
