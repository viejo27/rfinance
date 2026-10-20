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

	var MONTHS = ['ENE', 'FEB', 'MAR', 'ABR', 'MAY', 'JUN', 'JUL', 'AGO', 'SEP', 'OCT', 'NOV', 'DIC'];
	var MONTHS_LONG = ['ENERO', 'FEBRERO', 'MARZO', 'ABRIL', 'MAYO', 'JUNIO', 'JULIO', 'AGOSTO', 'SEPTIEMBRE', 'OCTUBRE', 'NOVIEMBRE', 'DICIEMBRE'];
	var reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

	function fmt(value, currency) {
		return (currency === 'USD' ? nfUSD : nfARS).format(value);
	}

	function amountText(value, currency) {
		return (value < 0 ? '−' : '') + fmt(Math.abs(value), currency) + ' ' + currency;
	}

	function shortDate(iso) {
		return iso.slice(8, 10) + ' ' + MONTHS[Number(iso.slice(5, 7)) - 1];
	}

	function monthIso(year, month) {
		return year + '-' + String(month).padStart(2, '0');
	}

	function el(tag, cls, text) {
		var n = document.createElement(tag);
		if (cls) n.className = cls;
		if (text !== undefined) n.textContent = text;
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

	async function request(url, options) {
		var r = await fetch(url, options);
		if (r.status === 401) {
			window.location.href = '/login';
			throw new Error('sesión expirada');
		}
		return r;
	}

	async function loadSnapshot() {
		var r = await request('/api/snapshot');
		if (!r.ok) throw new Error('el servidor no contesta el estado');
		snapshot = await r.json();
		if (!viewedMonth) viewedMonth = monthIso(snapshot.year, snapshot.month);
	}

	// ---------- estado ----------

	var snapshot = null; // del GET /api/estado
	var viewedMonth = null; // 'YYYY-MM' del mes visto en libro y presupuestos
	var pendingUndos = {}; // ids en ventana DESHACER (borrado diferido)
	var undoTimers = {};
	var sortState = { key: 'entry_date', dir: -1 };
	var editingTransactionId = null;
	var editingIsRecurring = false; // la fila en edición pertenece a una recurrencia
	var editingAccountId = null;
	var editingCategoryId = null;

	function isActive(m) {
		return !pendingUndos[m.id];
	}

	function isViewedMonth(iso) {
		return iso.slice(0, 7) === viewedMonth;
	}

	function isScheduled(iso) {
		return snapshot && iso > snapshot.today;
	}

	// serie anual ENE→DIC del año en curso, derivada de los asientos reales
	function yearSeries(currency) {
		var series = [null, null, null, null, null, null, null, null, null, null, null, null];
		if (!snapshot) return series;
		for (var i = 0; i < snapshot.transactions.length; i++) {
			var m = snapshot.transactions[i];
			if (!isActive(m) || m.currency !== currency) continue;
			if (Number(m.entry_date.slice(0, 4)) !== snapshot.year) continue;
			var month = Number(m.entry_date.slice(5, 7)) - 1;
			var par = series[month] || [0, 0];
			if (m.value > 0) par[0] += m.value;
			else par[1] += -m.value;
			series[month] = par;
		}
		return series;
	}

	function showNotice(text, cls) {
		var notice = $('entryStatus');
		notice.textContent = text;
		notice.classList.toggle('notice-danger', cls === 'notice-danger');
		notice.classList.toggle('notice-success', cls === 'notice-success');
		notice.hidden = false;
	}

	// ---------- banda de mando ----------

	function renderOverview() {
		var byCurrency = { ARS: 0, USD: 0 };
		var namesByCurrency = { ARS: [], USD: [] };
		for (var i = 0; i < snapshot.accounts.length; i++) {
			var c = snapshot.accounts[i];
			byCurrency[c.currency] += c.balance;
			namesByCurrency[c.currency].push(c.name);
		}
		$('totalArs').textContent = fmt(byCurrency.ARS, 'ARS');
		$('totalUsd').textContent = fmt(byCurrency.USD, 'USD');
		$('totalArs').closest('.overview-total').classList.toggle('total-negative', byCurrency.ARS < 0);
		$('totalUsd').closest('.overview-total').classList.toggle('total-negative', byCurrency.USD < 0);
		$('subArs').textContent = namesByCurrency.ARS.join(' · ') || 'sin cuentas ARS';
		$('subUsd').textContent = namesByCurrency.USD.join(' · ') || 'sin cuentas USD';

		$('historyYear').textContent = 'AÑO ' + snapshot.year;
		var series = yearSeries('ARS');
		var netoAnual = 0;
		for (var m = 0; m < 12; m++) {
			if (series[m]) netoAnual += series[m][0] - series[m][1];
		}
		$('netYearly').textContent = netoAnual ? amountText(netoAnual, 'ARS') + ' NETO' : 'SIN DATOS';
		var box = $('overviewHistory');
		box.innerHTML = '';
		box.appendChild(yearChart(series, 'history', 'Ingresos y egresos mensuales en pesos, año ' + snapshot.year, box.clientWidth));
	}

	function yearChart(series, cssClass, ariaLabel, W) {
		W = Math.max(320, Math.round(W || 660));
		var H = 240;
		var maxValue = 0;
		for (var m = 0; m < 12; m++) {
			if (series[m]) maxValue = Math.max(maxValue, series[m][0], series[m][1]);
		}
		var g = svg('svg', { class: cssClass, viewBox: '0 0 ' + W + ' ' + H, role: 'img', 'aria-label': ariaLabel });
		var pitch = W / 12;
		var base = H - 32;
		for (var m2 = 0; m2 < 12; m2++) {
			var x = m2 * pitch + pitch * 0.18;
			var par = series[m2];
			if (par) {
				bar(g, x, par[0], maxValue, 'var(--ocre-barra)', MONTHS[m2] + ' ingresos: ' + fmt(par[0], 'ARS'), base);
				bar(g, x + pitch * 0.36, par[1], maxValue, 'var(--rojo)', MONTHS[m2] + ' egresos: ' + fmt(par[1], 'ARS'), base);
			} else {
				g.appendChild(svg('rect', { x: x, y: base - 2, width: Math.max(8, pitch * 0.18), height: 2, fill: 'var(--concreto)' }));
				g.appendChild(svg('rect', { x: x + pitch * 0.36, y: base - 2, width: Math.max(8, pitch * 0.18), height: 2, fill: 'var(--concreto)' }));
			}
			var t = svg('text', { x: m2 * pitch + pitch / 2, y: H - 12, 'text-anchor': 'middle' });
			t.textContent = MONTHS[m2];
			g.appendChild(t);
		}
		g.appendChild(svg('line', { class: 'axis', x1: 4, y1: base, x2: W - 4, y2: base }));
		return g;
	}

	function bar(chart, x, value, maxValue, color, title, base) {
		var h = Math.max(3, Math.round((value / maxValue) * (base - 16)));
		var rect = svg('rect', { x: x, y: base - h, width: 12, height: h, fill: color });
		var t = svg('title');
		t.textContent = title;
		rect.appendChild(t);
		chart.appendChild(rect);
	}

	// ---------- cuentas ----------

	function renderAccounts() {
		var grid = $('accountsGrid');
		grid.innerHTML = '';
		for (var i = 0; i < snapshot.accounts.length; i++) {
			var c = snapshot.accounts[i];
			var block = el('div', 'account' + (c.currency === 'USD' ? ' account-grid' : ''));
			block.appendChild(el('span', 'mini-label', c.name));
			block.appendChild(el('span', 'account-balance' + (c.balance < 0 ? ' account-negative' : ''), fmt(c.balance, c.currency)));
			block.appendChild(el('span', 'account-chip', c.currency));
			var actions = el('div', 'account-actions');
			actions.appendChild(makeButton('EDITAR', function (acct) { return function () { enterAccountEdit(acct); }; }, c));
			actions.appendChild(makeButton('BORRAR', function (acct) { return function () { deleteAccount(acct); }; }, c, 'btn-mini-danger'));
			block.appendChild(actions);
			grid.appendChild(block);
		}
		$('accountsEmpty').hidden = snapshot.accounts.length !== 0;
		syncSelects();
	}

	function makeButton(text, factory, datum, extra) {
		var b = el('button', 'btn btn-mini' + (extra ? ' ' + extra : ''), text);
		b.type = 'button';
		b.addEventListener('click', factory(datum));
		return b;
	}

	async function mutate(url, method, body, successNotice) {
		var options = { method: method };
		if (body !== undefined) {
			options.headers = { 'Content-Type': 'application/json' };
			options.body = JSON.stringify(body);
		}
		var r = await request(url, options);
		if (!r.ok) {
			var text = await r.text();
			showNotice(text || 'No se pudo guardar', 'notice-danger');
			throw new Error(text);
		}
		await loadSnapshot();
		renderAll();
		if (successNotice) showNotice(successNotice, 'notice-success');
		return r;
	}

	async function deleteAccount(c) {
		var n = 0;
		for (var i = 0; i < snapshot.transactions.length; i++) {
			if (isActive(snapshot.transactions[i]) && snapshot.transactions[i].account === c.name) n++;
		}
		var msg = '¿Borrar la cuenta «' + c.name + '»?';
		if (n) msg += ' Sus ' + n + ' asiento(s) del libro también se borran.';
		if (!window.confirm(msg)) return;
		try {
			await mutate('/api/accounts/' + c.id, 'DELETE', undefined, 'Cuenta «' + c.name + '» borrada.');
		} catch (e) { /* el aviso ya quedó en pantalla */ }
	}

	function enterAccountEdit(c) {
		editingAccountId = c.id;
		$('accountFormTitle').textContent = 'EDITAR CUENTA';
		$('accountFormName').value = c.name;
		$('accountFormCurrency').value = c.currency;
		$('accountFormBalance').value = c.opening_balance;
		$('accountFormSubmit').textContent = 'GUARDAR';
		$('accountFormCancel').hidden = false;
		$('accountFormName').focus();
	}

	function exitAccountEdit() {
		editingAccountId = null;
		$('accountFormTitle').textContent = 'AGREGAR CUENTA';
		$('accountForm').reset();
		$('accountFormSubmit').textContent = 'AGREGAR';
		$('accountFormCancel').hidden = true;
	}

	$('accountForm').addEventListener('submit', async function (e) {
		e.preventDefault();
		var body = {
			name: $('accountFormName').value.trim(),
			currency: $('accountFormCurrency').value,
			opening_balance: parseFloat(($('accountFormBalance').value || '0').replace(',', '.')),
		};
		try {
			if (editingAccountId !== null) {
				await mutate('/api/accounts/' + editingAccountId, 'PATCH', body, 'Cuenta «' + body.name + '» guardada.');
				exitAccountEdit();
			} else {
				await mutate('/api/accounts', 'POST', body, 'Cuenta «' + body.name + '» agregada.');
				exitAccountEdit();
			}
		} catch (err) { /* aviso en pantalla */ }
	});

	$('accountFormCancel').addEventListener('click', exitAccountEdit);

	// ---------- navegador de mes ----------

	function renderMonthNav() {
		var parts = viewedMonth.split('-');
		var month = Number(parts[1]);
		var year = Number(parts[0]);
		$('monthViewed').textContent = MONTHS_LONG[month - 1] + ' ' + year;
		var esHoy = viewedMonth === monthIso(snapshot.year, snapshot.month);
		$('monthToday').hidden = esHoy;
		$('budgetMonth').hidden = esHoy;
		if (!esHoy) $('budgetMonth').textContent = 'MES VISTO: ' + MONTHS[month - 1] + ' ' + year;
	}

	$('monthPrev').addEventListener('click', function () { changeMonth(-1); });
	$('monthNext').addEventListener('click', function () { changeMonth(1); });

	function changeMonth(delta) {
		var parts = viewedMonth.split('-');
		var total = Number(parts[0]) * 12 + Number(parts[1]) - 1 + delta;
		viewedMonth = monthIso(Math.floor(total / 12), (total % 12 + 1));
		renderMonthNav();
		renderLedger();
		renderBudget();
	}

	$('monthToday').addEventListener('click', function () {
		viewedMonth = monthIso(snapshot.year, snapshot.month);
		renderMonthNav();
		renderLedger();
		renderBudget();
	});

	// ---------- libro contable (mes visto) ----------

	function compare(a, b) {
		var cmp;
		if (sortState.key === 'value') {
			cmp = a.value - b.value;
		} else {
			cmp = String(a[sortState.key]).localeCompare(String(b[sortState.key]), 'es');
		}
		return cmp * sortState.dir;
	}

	function renderLedger(newIds) {
		newIds = newIds || {};
		var body = $('ledgerBody');
		body.innerHTML = '';
		var currency = $('filterCurrency').value;
		var category = $('filterCategory').value;
		var description = $('filterConcept').value.trim().toLowerCase();

		var rows = [];
		for (var i = 0; i < snapshot.transactions.length; i++) {
			var m = snapshot.transactions[i];
			if (!isViewedMonth(m.entry_date)) continue;
			var ok = (!currency || m.currency === currency) &&
				(!category || m.category === category) &&
				(!description || m.description.toLowerCase().includes(description));
			if (ok) rows.push(m);
		}
		rows.sort(compare);

		for (var j = 0; j < rows.length; j++) {
			body.appendChild(ledgerRow(rows[j], newIds[rows[j].id]));
		}

		var sinAsientos = snapshot.transactions.length === 0;
		$('ledgerEmptyText').textContent = sinAsientos
			? 'EL LIBRO ESTÁ VACÍO · ANOTA TU PRIMER ASIENTO ARRIBA'
			: 'SIN ASIENTOS EN ESTE MES · NAVEGA OTRO MES O ANOTA ARRIBA';
		$('ledgerEmpty').hidden = rows.length !== 0;
		$('ledgerCount').textContent = rows.length + (rows.length === 1 ? ' ASIENTO' : ' ASIENTOS');
	}

	function ledgerRow(m, isNew) {
		var isVoided = !!pendingUndos[m.id];
		var tr = el('tr', 'transaction' + (isVoided ? ' row-void' : '') + (isNew ? ' row-new' : ''));
		var scheduled = isScheduled(m.entry_date);
		if (scheduled) tr.classList.add('row-scheduled');

		tr.appendChild(cell('cell-date', shortDate(m.entry_date) + (scheduled ? ' · PRÓX.' : '')));
		tr.appendChild(cell('cell-category', m.category));
		tr.appendChild(cell('cell-account', m.account));

		var tdC = el('td', 'cell-description');
		if (m.recurrence_id) {
			tdC.appendChild(el('span', 'stamp-demo-h recurrence-chip', 'RECURRENTE'));
		}
		tdC.appendChild(document.createTextNode(m.description));
		if (isNew && !reduceMotion && !isVoided) {
			var sticker = el('span', 'stamp', 'ANOTADO');
			sticker.setAttribute('aria-hidden', 'true');
			tdC.appendChild(sticker);
		}
		tr.appendChild(tdC);

		tr.appendChild(cell('cell-amount ' + (m.value < 0 ? 'expense' : 'income'), amountText(m.value, m.currency)));

		var tdA = el('td', 'row-actions');
		if (isVoided) {
			tdA.appendChild(makeButton('DESHACER', function () { return function () { undo(m.id); }; }, null, 'btn-undo'));
		} else {
			tdA.appendChild(makeButton('EDITAR', function (txn) { return function () { enterTransactionEdit(txn); }; }, m));
			tdA.appendChild(makeButton('BORRAR', function (txn) { return function () { deleteTransaction(txn); }; }, m, 'btn-mini-danger'));
			if (m.recurrence_id) {
				var bRec = el('button', 'btn btn-mini btn-mini-danger', 'BORRAR REC.');
				bRec.type = 'button';
				bRec.title = 'Corta la recurrencia: borra los asientos futuros';
				bRec.addEventListener('click', function () { deleteRecurrence(m.recurrence_id); });
				tdA.appendChild(bRec);
			}
		}
		tr.appendChild(tdA);
		return tr;
	}

	function cell(cls, text) {
		return el('td', cls, text);
	}

	function deleteTransaction(m) {
		var detail = '«' + m.description + '» (' + shortDate(m.entry_date) + ' · ' + amountText(m.value, m.currency) + ')';
		if (!window.confirm('¿Borrar el asiento ' + detail + '?')) return;
		pendingUndos[m.id] = true;
		undoTimers[m.id] = setTimeout(function () { finalizeDelete(m.id); }, 4000);
		renderLedger();
	}

	function undo(id) {
		clearTimeout(undoTimers[id]);
		delete undoTimers[id];
		delete pendingUndos[id];
		renderLedger();
	}

	async function finalizeDelete(id) {
		clearTimeout(undoTimers[id]);
		delete undoTimers[id];
		delete pendingUndos[id];
		try {
			await mutate('/api/transactions/' + id, 'DELETE', undefined, 'Asiento borrado.');
		} catch (e) { /* aviso en pantalla */ }
	}

	async function deleteRecurrence(recId) {
		var generated = 0;
		var name = 'recurrencia';
		for (var i = 0; i < snapshot.transactions.length; i++) {
			var m = snapshot.transactions[i];
			if (m.recurrence_id === recId) {
				if (isScheduled(m.entry_date)) generated++;
				name = m.description;
			}
		}
		if (!window.confirm('¿Borrar la recurrencia «' + name + '»? Se borran ' + generated + ' asiento(s) futuro(s); los ya cargados se conservan.')) return;
		try {
			await mutate('/api/recurrences/' + recId, 'DELETE', undefined, 'Recurrencia cortada; los asientos ya cargados quedan.');
		} catch (e) { /* aviso en pantalla */ }
	}

	// ---------- presupuestos / categorías (mes visto) ----------

	function categoryUsage(cat) {
		var used = 0;
		for (var i = 0; i < snapshot.transactions.length; i++) {
			var m = snapshot.transactions[i];
			if (!isActive(m) || !isViewedMonth(m.entry_date)) continue;
			if (m.category === cat.name && m.currency === cat.currency && m.value < 0) {
				used += -m.value;
			}
		}
		return used;
	}

	function renderBudget() {
		var lista = $('budgetList');
		lista.innerHTML = '';
		for (var i = 0; i < snapshot.categories.length; i++) {
			var cat = snapshot.categories[i];
			var used = categoryUsage(cat);
			var li = el('li', 'budget-item');
			if (cat.budget_limit === null) {
				li.classList.add('budget-no-limit');
				li.appendChild(el('span', 'budget-name', cat.name));
				li.appendChild(el('div', 'budget-bar budget-no-bar'));
				li.appendChild(el('span', 'budget-readout figure', fmt(used, cat.currency) + ' ' + cat.currency + ' este mes · SIN LÍMITE'));
			} else {
				var pct = Math.round((used / cat.budget_limit) * 100);
				var isOver = pct > 100;
				if (isOver) li.classList.add('budget-over');
				li.appendChild(el('span', 'budget-name', cat.name));
				var bar = el('div', 'budget-bar');
				bar.setAttribute('role', 'progressbar');
				bar.setAttribute('aria-valuenow', String(Math.min(pct, 100)));
				bar.setAttribute('aria-valuemin', '0');
				bar.setAttribute('aria-valuemax', '100');
				bar.setAttribute('aria-label', 'Ejecución de ' + cat.name);
				bar.style.setProperty('--pct', Math.min(pct, 100) + '%');
				li.appendChild(bar);
				var readout = el('span', 'budget-readout figure');
				readout.textContent = fmt(used, cat.currency) + ' / ' + fmt(cat.budget_limit, cat.currency) + ' ' + cat.currency + ' ';
				readout.appendChild(el('span', 'budget-pct', pct + '%'));
				if (isOver) readout.appendChild(el('span', 'tag-over-budget', 'EXCEDIDO'));
				readout.appendChild(document.createTextNode(' · ANUAL ' + fmt(cat.budget_limit * 12, cat.currency)));
				li.appendChild(readout);
			}
			var actions = el('div', 'budget-actions');
			actions.appendChild(makeButton('EDITAR', function (c2) { return function () { enterCategoryEdit(c2); }; }, cat));
			actions.appendChild(makeButton('BORRAR', function (c3) { return function () { deleteCategory(c3); }; }, cat, 'btn-mini-danger'));
			li.appendChild(actions);
			lista.appendChild(li);
		}
		$('budgetEmpty').hidden = snapshot.categories.length !== 0;
		renderMonthNav();
	}

	async function deleteCategory(cat) {
		var entryCount = 0;
		for (var i = 0; i < snapshot.transactions.length; i++) {
			if (snapshot.transactions[i].category === cat.name) entryCount++;
		}
		var msg = '¿Borrar la categoría «' + cat.name + '»?';
		if (entryCount) msg += ' Sus ' + entryCount + ' asiento(s) conservan la categoría como texto.';
		if (!window.confirm(msg)) return;
		try {
			await mutate('/api/categories/' + cat.id, 'DELETE', undefined, 'Categoría «' + cat.name + '» borrada.');
		} catch (e) { /* aviso en pantalla */ }
	}

	function enterCategoryEdit(cat) {
		editingCategoryId = cat.id;
		$('categoryFormTitle').textContent = 'EDITAR CATEGORÍA';
		$('categoryFormName').value = cat.name;
		$('categoryFormLimit').value = cat.budget_limit === null ? '' : cat.budget_limit;
		$('categoryFormCurrency').value = cat.currency;
		$('categoryFormSubmit').textContent = 'GUARDAR';
		$('categoryFormCancel').hidden = false;
		$('categoryFormName').focus();
	}

	function exitCategoryEdit() {
		editingCategoryId = null;
		$('categoryFormTitle').textContent = 'AGREGAR CATEGORÍA';
		$('categoryForm').reset();
		$('categoryFormSubmit').textContent = 'AGREGAR';
		$('categoryFormCancel').hidden = true;
	}

	$('categoryForm').addEventListener('submit', async function (e) {
		e.preventDefault();
		var limitText = ($('categoryFormLimit').value || '').replace(',', '.');
		var body = {
			name: $('categoryFormName').value.trim(),
			budget_limit: limitText === '' ? null : parseFloat(limitText),
			currency: $('categoryFormCurrency').value,
		};
		try {
			if (editingCategoryId !== null) {
				await mutate('/api/categories/' + editingCategoryId, 'PATCH', body, 'Categoría «' + body.name + '» guardada.');
				exitCategoryEdit();
			} else {
				await mutate('/api/categories', 'POST', body, 'Categoría «' + body.name + '» agregada.');
				exitCategoryEdit();
			}
		} catch (err) { /* aviso en pantalla */ }
	});

	$('categoryFormCancel').addEventListener('click', exitCategoryEdit);

	// ---------- carteles (12 meses del año, derivados de asientos reales) ----------

	function renderCards() {
		var grid = $('cardsGrid');
		grid.innerHTML = '';
		var Wc = Math.max(320, Math.round((grid.clientWidth - 22) / 2));
		['ARS', 'USD'].forEach(function (currency) {
			var series = yearSeries(currency);
			var incomeTotal = 0;
			var expenseTotal = 0;
			for (var m = 0; m < 12; m++) {
				if (series[m]) {
					incomeTotal += series[m][0];
					expenseTotal += series[m][1];
				}
			}
			var card = el('article', 'card');
			var titles = el('div', 'card-titles');
			var head = el('span', 'card-head');
			head.appendChild(el('small', null, 'AÑO ' + snapshot.year + ' · ' + currency));
			head.appendChild(document.createTextNode('ING ' + fmt(incomeTotal, currency) + ' · GAS ' + fmt(expenseTotal, currency)));
			titles.appendChild(head);
			card.appendChild(titles);
			card.appendChild(yearChart(series, 'chart', 'Ingresos y egresos mensuales en ' + currency + ', año ' + snapshot.year, Wc));
			var legend = el('div', 'legend mini-label');
			var l1 = el('span');
			l1.appendChild(el('i', 'legend-income'));
			l1.appendChild(document.createTextNode('INGRESOS'));
			var l2 = el('span');
			l2.appendChild(el('i', 'legend-expense'));
			l2.appendChild(document.createTextNode('EGRESOS'));
			legend.appendChild(l1);
			legend.appendChild(l2);
			if (incomeTotal === 0 && expenseTotal === 0) {
				legend.appendChild(el('span', 'legend-note', 'SIN DATOS AÚN · CARGA ASIENTOS Y VOLVÉ'));
			}
			card.appendChild(legend);
			grid.appendChild(card);
		});
	}

	// ---------- captura: alta o modo edición, con recurrencia ----------

	function syncSelects() {
		var accountSel = $('entryAccount');
		var current = accountSel.value;
		accountSel.innerHTML = '';
		for (var i = 0; i < snapshot.accounts.length; i++) {
			var o = el('option', null, snapshot.accounts[i].name + ' · ' + snapshot.accounts[i].currency);
			o.value = String(snapshot.accounts[i].id);
			accountSel.appendChild(o);
		}
		if (current) accountSel.value = current;

		var catSel = $('entryCategory');
		var fSel = $('filterCategory');
		var c2 = catSel.value;
		var f2 = fSel.value;
		catSel.innerHTML = '';
		fSel.innerHTML = '';
		var allOption = el('option', null, 'TODAS CATEGORÍAS');
		allOption.value = '';
		fSel.appendChild(allOption);
		for (var j = 0; j < snapshot.categories.length; j++) {
			var name = snapshot.categories[j].name;
			// el form manda category_id (UUID); el filtro sigue filtrando por nombre
			var o2 = el('option', null, name.toUpperCase());
			o2.value = String(snapshot.categories[j].id);
			catSel.appendChild(o2);
			var o3 = el('option', null, name.toUpperCase());
			o3.value = name;
			fSel.appendChild(o3);
		}
		if (c2) catSel.value = c2;
		if (f2) fSel.value = f2;
	}

	function renderAll(newIds) {
		renderOverview();
		renderAccounts();
		renderBudget();
		renderLedger(newIds);
		renderCards();
		renderMonthNav();
		syncSelects();
	}

	$('entryForm').addEventListener('submit', async function (e) {
		e.preventDefault();

		var entry_date = $('entryDate').value;
		var amount = parseFloat(($('entryAmount').value || '').replace(',', '.'));
		var description = $('entryDescription').value.trim();
		var accountId = $('entryAccount').value;
		var catId = $('entryCategory').value || null;
		var currency = $('entryCurrency').value;
		var isExpense = document.querySelector('input[name="entryType"]:checked').value === 'expense';
		var recurrence = $('entryRecurring').checked;
		var installmentsText = $('entryInstallments').value;
		var installments = installmentsText === '' ? null : parseInt(installmentsText, 10);

		if (!entry_date || !description || !Number.isFinite(amount) || amount <= 0) {
			showNotice('Falta fecha, concepto o un monto mayor que cero. Corrige y anota de nuevo.', 'notice-danger');
			return;
		}
		if (!accountId) {
			showNotice('Primero agrega una cuenta en la sección CUENTAS.', 'notice-danger');
			return;
		}

		var body = {
			entry_date: entry_date,
			account_id: accountId,
			category_id: catId,
			description: description,
			amount: amount,
			is_income: !isExpense,
			currency: currency,
		};
		if (recurrence) body.recurrence = { installment_count: installments };

		try {
			var r;
			if (editingTransactionId !== null) {
				body.scope = selectedScope();
				r = await request('/api/transactions/' + editingTransactionId, {
					method: 'PATCH',
					headers: { 'Content-Type': 'application/json' },
					body: JSON.stringify(body),
				});
			} else {
				r = await request('/api/transactions', {
					method: 'POST',
					headers: { 'Content-Type': 'application/json' },
					body: JSON.stringify(body),
				});
			}
			if (!r.ok) {
				showNotice((await r.text()) || 'No se pudo registrar', 'notice-danger');
				return;
			}
			var data = await r.json();
			var newIds = {};
			newIds[data.id] = true;
			await loadSnapshot();
			renderAll(newIds);

			if (editingTransactionId !== null) {
				editingTransactionId = null;
				resetEntryForm();
				showNotice(
					data.generated > 0
						? 'Asiento guardado + ' + data.generated + ' asiento(s) siguiente(s) actualizado(s).'
						: 'Asiento guardado.',
					'notice-success'
				);
			} else {
				var msg = data.generated > 0
					? 'Asiento anotado + ' + data.generated + ' asiento(s) futuro(s) ' + (installments === null ? 'de la recurrencia' : 'de las ' + installments + ' cuotas') + '.'
					: 'Asiento anotado en el libro.';
				resetEntryForm();
				showNotice(msg, 'notice-success');
				$('entryAmount').focus();
			}
		} catch (err) {
			showNotice('Error de conexión con el servidor. Intenta de nuevo.', 'notice-danger');
		}
	});

	function resetEntryForm() {
		$('entryTitle').textContent = 'ANOTAR EN EL LIBRO';
		$('entrySubmit').textContent = 'REGISTRAR';
		$('entryCancel').hidden = true;
		$('entryForm').classList.remove('is-editing');
		$('entryForm').reset();
		editingIsRecurring = false;
		$('entryScope').classList.remove('is-scope-on');
		$('entryRecurring').disabled = false;
		$('entryInstallments').disabled = false;
		$('entryDate').value = snapshot.today;
		syncRecurringRow();
	}

	function enterTransactionEdit(m) {
		editingTransactionId = m.id;
		$('entryTitle').textContent = 'EDITAR ASIENTO';
		$('entryDate').value = m.entry_date;
		$('entryAmount').value = String(Math.abs(m.value));
		$('entryCurrency').value = m.currency;
		document.querySelector('input[name="entryType"][value="' + (m.value < 0 ? 'expense' : 'income') + '"]').checked = true;
		syncSelects();
		$('entryAccount').value = String(m.account_id);
		$('entryCategory').value = m.category_id === null ? '' : String(m.category_id);
		$('entryDescription').value = m.description;
		var recurrence = null;
		for (var i = 0; i < snapshot.recurrences.length; i++) {
			if (snapshot.recurrences[i].id === m.recurrence_id) recurrence = snapshot.recurrences[i];
		}
		editingIsRecurring = !!m.recurrence_id;
		$('entryScope').classList.toggle('is-scope-on', editingIsRecurring);
		document.querySelector('input[name="entryScope"][value="single"]').checked = true;
		$('entryRecurring').checked = !!recurrence;
		$('entryInstallments').value = recurrence && recurrence.installments !== null ? String(recurrence.installments) : '';
		syncRecurringRow();
		syncScopeControls();
		$('entrySubmit').textContent = 'GUARDAR';
		$('entryCancel').hidden = false;
		$('entryForm').classList.add('is-editing');
		$('entryAmount').focus();
	}

	$('entryCancel').addEventListener('click', function () {
		editingTransactionId = null;
		resetEntryForm();
	});

	$('entryRecurring').addEventListener('change', syncRecurringRow);

	function syncRecurringRow() {
		$('entryRecurring').closest('.entry-recurring').classList.toggle('is-recurring-on', $('entryRecurring').checked);
	}

	// ---------- alcance de la edición de una fila recurrente ----------

	function selectedScope() {
		var r = document.querySelector('input[name="entryScope"]:checked');
		return r ? r.value : 'single';
	}

	function syncScopeControls() {
		// con "solo este asiento" la serie no se toca: se congelan sus controles
		var isFrozen = editingIsRecurring && selectedScope() === 'single';
		$('entryRecurring').disabled = isFrozen;
		$('entryInstallments').disabled = isFrozen;
	}

	document.querySelectorAll('input[name="entryScope"]').forEach(function (r) {
		r.addEventListener('change', syncScopeControls);
	});

	// ---------- orden ----------

	$('ledgerTable').querySelectorAll('th[data-sort-by] button').forEach(function (btn) {
		btn.addEventListener('click', function () {
			var th = btn.closest('th');
			var key = th.dataset.sortBy;
			if (sortState.key === key) {
				sortState.dir = -sortState.dir;
			} else {
				sortState.key = key;
				sortState.dir = -1;
			}
			var ths = $('ledgerTable').querySelectorAll('th[data-sort-by]');
			for (var i = 0; i < ths.length; i++) {
				ths[i].setAttribute('aria-sort', ths[i].dataset.sortBy === sortState.key ? (sortState.dir === 1 ? 'ascending' : 'descending') : 'none');
			}
			renderLedger();
		});
	});

	// ---------- filtros ----------

	$('filterCurrency').addEventListener('change', function () { renderLedger(); });
	$('filterCategory').addEventListener('change', function () { renderLedger(); });
	$('filterConcept').addEventListener('input', function () { renderLedger(); });
	$('filterClear').addEventListener('click', function () {
		$('filterCurrency').value = '';
		$('filterCategory').value = '';
		$('filterConcept').value = '';
		renderLedger();
	});

	// ---------- accesos de estados vacíos ----------

	$('accountsEmptyBtn').addEventListener('click', function () { $('accountFormName').focus(); });
	$('budgetEmptyBtn').addEventListener('click', function () { $('categoryFormName').focus(); });

	// ---------- charts responsivos ----------

	var resizePending = false;
	window.addEventListener('resize', function () {
		if (resizePending) return;
		resizePending = true;
		requestAnimationFrame(function () {
			resizePending = false;
			if (snapshot) {
				renderOverview();
				renderCards();
			}
		});
	});

	// ---------- logout ----------

	$('logoutBtn').addEventListener('click', async function () {
		await fetch('/api/logout', { method: 'POST' });
		window.location.href = '/login';
	});

	// ---------- arranque ----------

	loadSnapshot()
		.then(function () { renderAll(); })
		.catch(function (e) {
			if (String(e.message) !== 'sesión expirada') {
				var frame = document.querySelector('main');
				var box = el('div', 'empty-state');
				box.appendChild(el('span', 'mini-label', 'EL SERVIDOR NO CONTESTA EL ESTADO · RECARGA LA PÁGINA'));
				if (frame) frame.appendChild(box);
			}
		});
})();
