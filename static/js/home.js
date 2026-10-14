// RFinance — home: carga rápida (El Sello), orden y filtros del libro.
// Datos de ejemplo: nada se persiste, todo vive en memoria de la pestaña.

const libro = document.getElementById('libroTabla');
const cuerpo = document.getElementById('libroCuerpo');
const vacio = document.getElementById('libroVacio');
const conteo = document.getElementById('libroConteo');

const fMoneda = document.getElementById('fMoneda');
const fCategoria = document.getElementById('fCategoria');
const fConcepto = document.getElementById('fConcepto');
const fLimpiar = document.getElementById('fLimpiar');

const nfARS = new Intl.NumberFormat('es-AR', { maximumFractionDigits: 0 });
const nfUSD = new Intl.NumberFormat('es-AR', { minimumFractionDigits: 2, maximumFractionDigits: 2 });

let filas = Array.from(cuerpo.querySelectorAll('tr.movimiento'));
let orden = { clave: 'fecha', dir: -1 };

const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

function formatear(valor, moneda) {
	return (moneda === 'USD' ? nfUSD : nfARS).format(valor);
}

function celda(clase, texto) {
	const td = document.createElement('td');
	if (clase) td.className = clase;
	td.textContent = texto;
	return td;
}

function fechaCorta(iso) {
	const partes = iso.split('-');
	const meses = ['ENE', 'FEB', 'MAR', 'ABR', 'MAY', 'JUN', 'JUL', 'AGO', 'SEP', 'OCT', 'NOV', 'DIC'];
	return partes[2] + ' ' + meses[Number(partes[1]) - 1];
}

// ---------- filtros ----------

function aplicarFiltros() {
	const moneda = fMoneda.value;
	const categoria = fCategoria.value;
	const concepto = fConcepto.value.trim().toLowerCase();

	cuerpo.innerHTML = '';
	let visibles = 0;
	for (const fila of filas) {
		const ok =
			(!moneda || fila.dataset.moneda === moneda) &&
			(!categoria || fila.dataset.categoria === categoria) &&
			(!concepto || fila.dataset.concepto.toLowerCase().includes(concepto));
		if (ok) {
			cuerpo.appendChild(fila);
			visibles++;
		}
	}
	vacio.hidden = visibles !== 0;
	conteo.textContent = visibles + (visibles === 1 ? ' ASIENTO' : ' ASIENTOS');
}

function limpiarFiltros() {
	fMoneda.value = '';
	fCategoria.value = '';
	fConcepto.value = '';
	aplicarFiltros();
}

fMoneda.addEventListener('change', aplicarFiltros);
fCategoria.addEventListener('change', aplicarFiltros);
fConcepto.addEventListener('input', aplicarFiltros);
fLimpiar.addEventListener('click', limpiarFiltros);
document.getElementById('vacioLimpiar').addEventListener('click', function (e) {
	e.preventDefault();
	limpiarFiltros();
});

// ---------- orden ----------

function ordenar() {
	filas.sort(function (a, b) {
		const va = a.dataset[orden.clave];
		const vb = b.dataset[orden.clave];
		let cmp;
		if (orden.clave === 'valor') {
			cmp = Number(va) - Number(vb);
		} else {
			cmp = String(va).localeCompare(String(vb), 'es');
		}
		return cmp * orden.dir;
	});
	libro.querySelectorAll('th[data-ordena]').forEach(function (th) {
		if (th.dataset.ordena === orden.clave) {
			th.setAttribute('aria-sort', orden.dir === 1 ? 'ascending' : 'descending');
		} else {
			th.setAttribute('aria-sort', 'none');
		}
	});
	aplicarFiltros();
}

libro.querySelectorAll('th[data-ordena] button').forEach(function (btn) {
	btn.addEventListener('click', function () {
		const th = btn.closest('th');
		const clave = th.dataset.ordena;
		if (orden.clave === clave) {
			orden.dir = -orden.dir;
		} else {
			orden.clave = clave;
			orden.dir = -1;
		}
		ordenar();
	});
});

// ---------- carga rápida: El Sello ----------

const captura = document.getElementById('capturaForm');
const estado = document.getElementById('capturaEstado');
const totalArs = document.getElementById('totalArs');
const totalUsd = document.getElementById('totalUsd');

function recalcularTotales(monedaTocada) {
	let ars = 0;
	let usd = 0;
	for (const fila of filas) {
		const v = Number(fila.dataset.valor);
		if (fila.dataset.moneda === 'USD') {
			usd += v;
		} else {
			ars += v;
		}
	}
	totalArs.textContent = formatear(ars, 'ARS');
	totalUsd.textContent = formatear(usd, 'USD');
	totalArs.closest('.mando-total').classList.toggle('total-neg', ars < 0);

	if (!reduceMotion && monedaTocada) {
		const bloque = monedaTocada === 'USD' ? totalUsd : totalArs;
		const caja = bloque.closest('.mando-total');
		caja.classList.remove('total-flash');
		void caja.offsetWidth; // reinicia la animación de corte duro
		caja.classList.add('total-flash');
	}
}

captura.addEventListener('submit', function (e) {
	e.preventDefault();

	const fecha = document.getElementById('capFecha').value;
	const monto = parseFloat(document.getElementById('capMonto').value);
	const concepto = document.getElementById('capConcepto').value.trim();
	const cuenta = document.getElementById('capCuenta').value;
	const categoria = document.getElementById('capCategoria').value;
	const moneda = document.getElementById('capMoneda').value;
	const egreso = document.querySelector('input[name="capTipo"]:checked').value === 'egreso';

	estado.hidden = false;
	if (!fecha || !concepto || !Number.isFinite(monto) || monto <= 0) {
		estado.textContent = 'Falta fecha, concepto o un monto mayor que cero. Corrige y anota de nuevo.';
		estado.classList.add('aviso-error');
		estado.classList.remove('aviso-exito');
		return;
	}
	estado.classList.remove('aviso-error');

	const valor = egreso ? -monto : monto;
	const fila = document.createElement('tr');
	fila.className = 'movimiento fila-nueva';
	fila.dataset.fecha = fecha;
	fila.dataset.categoria = categoria;
	fila.dataset.cuenta = cuenta;
	fila.dataset.concepto = concepto;
	fila.dataset.valor = String(valor);
	fila.dataset.moneda = moneda;
	fila.dataset.egreso = String(egreso);

	fila.appendChild(celda('celda-fecha', fechaCorta(fecha)));
	fila.appendChild(celda('celda-cat', categoria));
	fila.appendChild(celda('celda-cuenta', cuenta));

	const tdConcepto = celda('celda-concepto', '');
	const etiqueta = document.createElement('span');
	etiqueta.className = 'sello-demo-h';
	etiqueta.textContent = 'DEMO';
	tdConcepto.appendChild(etiqueta);
	tdConcepto.appendChild(document.createTextNode(concepto));
	if (!reduceMotion) {
		const marca = document.createElement('span');
		marca.className = 'estampa';
		marca.textContent = 'ANOTADO';
		marca.setAttribute('aria-hidden', 'true');
		tdConcepto.appendChild(marca);
	}
	fila.appendChild(tdConcepto);

	const montoTxt = (egreso ? '−' : '') + formatear(Math.abs(valor), moneda) + ' ' + moneda;
	fila.appendChild(celda('celda-monto ' + (egreso ? 'egreso' : 'ingreso'), montoTxt));

	filas = [fila].concat(filas);
	orden.clave = 'fecha';
	orden.dir = -1;
	ordenar();
	recalcularTotales(moneda);

	estado.textContent = 'Asiento anotado en el libro. Datos de ejemplo: no se guarda.';
	document.getElementById('capMonto').value = '';
	document.getElementById('capConcepto').value = '';
	document.getElementById('capMonto').focus();
});

// ---------- logout ----------

document.getElementById('logoutBtn').addEventListener('click', async function () {
	await fetch('/api/logout', {
		method: 'POST'
	});
	window.location.href = '/login';
});

// arranque
aplicarFiltros();
