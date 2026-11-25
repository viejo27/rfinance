// RFinance — inversiones: cliente del API de la cartera.
// El estado vive en la base; el GET /api/portfolio devuelve criterios,
// activos, diagnóstico y tasas. Toda mutación responde el estado completo
// recalculado, así el render tiene una sola fuente de verdad.

(function () {
	'use strict';

	var nfARS = new Intl.NumberFormat('es-AR', { maximumFractionDigits: 0 });
	var nfUSD = new Intl.NumberFormat('es-AR', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
	var nf1 = new Intl.NumberFormat('es-AR', { minimumFractionDigits: 1, maximumFractionDigits: 1 });
	var nfRate = new Intl.NumberFormat('es-AR', { minimumFractionDigits: 2, maximumFractionDigits: 2 });

	var state = null;
	var editingAssetId = null;
	var selectedTriggerAssetId = null;
	var targetPreview = null;
	var targetPreviewRequest = 0;
	var noticeTimer = null;
	var sectionTimers = {};

	function $(id) {
		return document.getElementById(id);
	}

	function el(tag, cls, text) {
		var node = document.createElement(tag);
		if (cls) node.className = cls;
		if (text !== undefined) node.textContent = text;
		return node;
	}

	function button(text, cls, onClick) {
		var node = el('button', cls, text);
		node.type = 'button';
		node.addEventListener('click', onClick);
		return node;
	}

	function fmt(value, currency) {
		if (value === null || value === undefined || !isFinite(value)) return '—';
		return (currency === 'USD' ? nfUSD : nfARS).format(value);
	}

	function pct(value) {
		if (value === null || value === undefined || !isFinite(value)) return '—';
		return nf1.format(value) + '\u00A0%';
	}

	function pp(value) {
		if (value === null || value === undefined || !isFinite(value)) return '—';
		var sign = value > 0 ? '+' : (value < 0 ? '−' : '');
		return sign + nf1.format(Math.abs(value)) + '\u00A0pp';
	}

	function num1(value) {
		return nf1.format(value);
	}

	function chipNum(value) {
		return Number.isInteger(value) ? String(value) : nf1.format(value);
	}

	function showNotice(text, kind) {
		var notice = $('portfolioNotice');
		notice.textContent = text;
		notice.hidden = false;
		notice.classList.toggle('notice-danger', kind === 'danger');
		notice.classList.toggle('notice-success', kind === 'success');
		if (noticeTimer) clearTimeout(noticeTimer);
		if (kind === 'success') {
			noticeTimer = setTimeout(function () {
				notice.hidden = true;
			}, 4000);
		}
	}

	function showSectionNotice(id, text, kind) {
		var notice = $(id);
		if (!notice) return;
		notice.textContent = text;
		notice.hidden = false;
		notice.classList.toggle('notice-danger', kind === 'danger');
		notice.classList.toggle('notice-success', kind === 'success');
		if (sectionTimers[id]) clearTimeout(sectionTimers[id]);
		if (kind === 'success') {
			sectionTimers[id] = setTimeout(function () {
				notice.hidden = true;
			}, 4000);
		}
	}

	async function request(url, options) {
		var response = await fetch(url, options);
		if (response.status === 401) {
			window.location.href = '/login';
			throw new Error('sesión expirada');
		}
		return response;
	}

	async function mutate(url, method, body, renderSettings, statusId) {
		try {
			var options = { method: method };
			if (body !== undefined) {
				options.headers = { 'Content-Type': 'application/json' };
				options.body = JSON.stringify(body);
			}
			var response = await request(url, options);
			if (!response.ok) {
				var text = await response.text();
				if (statusId) showSectionNotice(statusId, text || 'No se pudo guardar', 'danger');
				else showNotice(text || 'No se pudo guardar', 'danger');
				return false;
			}
			var next = await response.json();
			applyState(next, renderSettings === true);
			return true;
		} catch (error) {
			if (error.message !== 'sesión expirada') {
				if (statusId) showSectionNotice(statusId, 'No se pudo conectar con el servidor', 'danger');
				else showNotice('No se pudo conectar con el servidor', 'danger');
			}
			return false;
		}
	}

	function applyState(next, renderSettings) {
		state = next;
		renderOptions(renderSettings);
		renderFx();
		renderHero();
		renderChecks();
		renderComposition();
		renderAssets();
		renderRebalance();
		renderTargets();
		renderTriggers();
	}

	function fillSelect(select, options, selected) {
		select.textContent = '';
		for (var i = 0; i < options.length; i++) {
			var option = document.createElement('option');
			option.value = options[i].id;
			option.textContent = options[i].label;
			select.appendChild(option);
		}
		if (selected) select.value = selected;
		if (!select.value && options.length) select.value = options[0].id;
	}

	function optionLabel(options, id) {
		for (var i = 0; i < options.length; i++) {
			if (options[i].id === id) return options[i].label;
		}
		return id;
	}

	function classCss(id) {
		if (id === 'fixed_income') return 'rf';
		if (id === 'equity') return 'rv';
		return 'mix';
	}

	function renderOptions(renderSettings) {
		var keepClass = $('assetClass').value;
		var keepEconomy = $('assetEconomy').value;
		var keepSector = $('assetSector').value;
		var keepKind = $('triggerKind').value;

		fillSelect($('assetClass'), state.options.asset_classes, keepClass || 'equity');
		fillSelect($('assetEconomy'), state.options.economies, keepEconomy || 'us');
		fillSelect($('assetSector'), state.options.sectors, keepSector || 'technology');
		fillSelect($('triggerKind'), state.options.trigger_kinds, keepKind || 'thesis_broken');
		fillSelect($('settingsValuation'), state.options.valuations, state.settings.valuation);
		fillSelect($('valuationSelect'), state.options.valuations, state.fx.valuation);

		var switchNode = $('profileSwitch');
		switchNode.textContent = '';
		for (var i = 0; i < state.profiles.length; i++) {
			(function (profile) {
				var btn = button(profile.label, 'profile-btn', function () {
					applyProfile(profile.id);
				});
				btn.setAttribute('data-profile', profile.id);
				btn.setAttribute('aria-pressed', profile.id === state.settings.risk_profile ? 'true' : 'false');
				switchNode.appendChild(btn);
			})(state.profiles[i]);
		}

		if (renderSettings) renderSettingsValues();
	}

	function renderFx() {
		var fx = state.fx;
		$('valuationSelect').value = fx.valuation;
		$('fxMep').textContent = fx.mep === null ? '—' : nfRate.format(fx.mep);
		$('fxCcl').textContent = fx.ccl === null ? '—' : nfRate.format(fx.ccl);

		if (fx.fetched_at === null) {
			$('fxStamp').textContent = 'SIN LECTURA';
		} else {
			var date = new Date(fx.fetched_at * 1000);
			var stamp = String(date.getHours()).padStart(2, '0') + ':' + String(date.getMinutes()).padStart(2, '0');
			$('fxStamp').textContent = 'LECTURA ' + stamp + (fx.stale ? ' · VIEJA' : '');
		}

		var warning = $('fxWarning');
		if (fx.error) {
			warning.textContent = 'TASAS NO DISPONIBLES · ' + fx.error + ' · los activos en otra moneda quedan sin valuar.';
			warning.hidden = false;
		} else if (fx.stale) {
			warning.textContent = 'TASA VIEJA · DolarAPI no respondió; se mantiene la última lectura guardada.';
			warning.hidden = false;
		} else {
			warning.hidden = true;
		}
	}

	function renderHero() {
		var analysis = state.analysis;
		var stamp = $('scoreStamp');
		stamp.classList.remove('is-alert', 'is-empty');

		if (!analysis) {
			$('scoreValue').textContent = '—';
			$('scoreStatus').textContent = 'SIN VALUAR';
			stamp.classList.add('is-empty');
			$('heroTotal').textContent = '—';
			$('heroSub').textContent = '';
			$('statusDetail').textContent = state.analysis_error || 'Faltan datos para valuar la cartera.';
			$('kpiAssets').textContent = state.assets.filter(function (asset) {
				return asset.asset_class === 'equity';
			}).length;
			$('kpiAssetsSub').textContent = 'total ' + state.assets.length + ' · RV';
			$('kpiEconomies').textContent = '—';
			$('kpiEconomiesSub').textContent = 'sin valuar';
			$('kpiEffective').textContent = '—';
			$('kpiEffectiveSub').textContent = 'sin valuar';
			$('kpiFixed').textContent = '—';
			$('kpiFixedSub').textContent = 'sin valuar';
			$('portfolioEmpty').hidden = state.assets.length !== 0;
			return;
		}

		$('scoreValue').textContent = analysis.score;
		$('scoreStatus').textContent = analysis.status.toUpperCase();
		if (analysis.status === 'Sin datos' || analysis.status === 'Sin valuar') {
			stamp.classList.add('is-empty');
		} else if (analysis.score < 50) {
			stamp.classList.add('is-alert');
		}

		$('heroView').textContent = state.fx.display_currency + ' · ' + state.fx.rate_kind;
		$('heroTotal').textContent = fmt(analysis.total_display, state.fx.display_currency);
		fitTotal($('heroTotal'));

		if (state.fx.display_currency === 'USD') {
			$('heroSub').textContent = analysis.total_ars === null
				? 'Sin tasa para expresar el total en ARS'
				: 'ARS ' + fmt(analysis.total_ars, 'ARS');
		} else {
			$('heroSub').textContent = analysis.total_usd === null
				? 'Sin tasa para expresar el total en USD'
				: 'USD ' + fmt(analysis.total_usd, 'USD');
		}

		$('statusDetail').textContent = analysis.status_detail;
		$('kpiAssets').textContent = analysis.n_equity;
		$('kpiAssetsSub').textContent = 'total ' + analysis.n_assets + ' · rango '
			+ state.settings.min_assets + '–' + state.settings.max_assets;
		$('kpiEconomies').textContent = analysis.n_economies;
		$('kpiEconomiesSub').textContent = analysis.n_economies
			? 'la más pesada: ' + analysis.max_economy_label + ' ' + pct(analysis.max_economy_pct)
			: 'sin datos';
		$('kpiEffective').textContent = num1(analysis.effective_units);
		$('kpiEffectiveSub').textContent = 'de ' + analysis.n_assets + ' activos por peso';
		$('kpiFixed').textContent = pct(analysis.rf_pct);
		$('kpiFixedSub').textContent = 'objetivo ' + pct(analysis.rf_target_pct)
			+ (analysis.mixed_pct > 0 ? ' · mixtos ' + pct(analysis.mixed_pct) : ' · sin mixtos');

		$('portfolioEmpty').hidden = state.assets.length !== 0;
	}

	function fitTotal(node) {
		node.style.fontSize = '';
		var min = 26;
		var guard = 0;
		while (node.scrollWidth > node.clientWidth && guard < 24) {
			var current = parseFloat(window.getComputedStyle(node).fontSize);
			if (current <= min) break;
			node.style.fontSize = (current - 2) + 'px';
			guard++;
		}
	}

	function renderChecks() {
		var list = $('checksList');
		list.textContent = '';
		if (!state.analysis || state.analysis.status === 'Sin datos' || state.analysis.status === 'Sin valuar') {
			var block = el('article', 'portfolio-check is-wide');
			var head = el('div', 'check-head');
			var noAssets = state.assets.length === 0;
			head.appendChild(el('h3', 'check-title', noAssets ? 'Sin datos' : 'Sin valuar'));
			head.appendChild(el('span', 'check-chip ' + (noAssets ? 'review' : 'alert'), noAssets ? 'SIN DATOS' : 'ALERTA'));
			block.appendChild(head);
			block.appendChild(el('p', 'check-detail', state.analysis_error
				|| (state.assets.length ? 'Los activos cargados no tienen valor para medir.' : 'Todavía no hay activos cargados.')));
			list.appendChild(block);
			return;
		}
		for (var i = 0; i < state.analysis.checks.length; i++) {
			var check = state.analysis.checks[i];
			var article = el('article', 'portfolio-check');
			var header = el('div', 'check-head');
			header.appendChild(el('h3', 'check-title', check.title));
			header.appendChild(el('span', 'check-chip ' + check.state, check.state_label));
			article.appendChild(header);
			article.appendChild(el('p', 'check-detail', check.detail));
			if (check.note) article.appendChild(el('p', 'check-note', check.note));
			list.appendChild(article);
		}
	}

	function barFillClass(slice, kind) {
		if (kind === 'class') return slice.id === 'fixed_income' ? 'rf' : 'rv';
		if (kind === 'nature') {
			if (slice.id === 'cyclical') return 'cyc';
			if (slice.id === 'defensive') return 'def';
			return 'mix';
		}
		return '';
	}

	function compositionBlock(title, sub, slices, options) {
		var block = el('article', 'composition-block');
		block.appendChild(el('h3', '', title));
		if (sub) block.appendChild(el('p', 'composition-sub', sub));
		if (!slices || !slices.length) {
			block.appendChild(el('p', 'mini-label', 'SIN DATOS'));
			if (options.note) block.appendChild(el('p', 'composition-note', options.note));
			return block;
		}
		var list = el('ul', 'bars');
		for (var i = 0; i < slices.length; i++) {
			(function (slice) {
				var row = el('li', 'bar-row');
				if (options.isOver && options.isOver(slice)) row.classList.add('is-over');
				row.appendChild(el('span', 'bar-label', slice.label));
				var track = el('span', 'bar-track');
				var fill = el('i', 'bar-fill ' + barFillClass(slice, options.kind));
				fill.style.width = Math.max(0, Math.min(100, slice.pct)) + '%';
				track.appendChild(fill);
				if (options.target) {
					var target = options.target(slice);
					if (target !== null && target !== undefined) {
						var mark = el('span', 'bar-target');
						mark.style.left = Math.max(0, Math.min(100, target)) + '%';
						track.appendChild(mark);
					}
				}
				row.appendChild(track);
				row.appendChild(el('span', 'bar-value', pct(slice.pct)));
				list.appendChild(row);
			})(slices[i]);
		}
		block.appendChild(list);
		if (options.note) block.appendChild(el('p', 'composition-note', options.note));
		return block;
	}

	function renderComposition() {
		var grid = $('compositionGrid');
		grid.textContent = '';
		var analysis = state.analysis;
		if (!analysis || analysis.status === 'Sin datos' || analysis.status === 'Sin valuar') {
			var empty = el('article', 'composition-block is-wide');
			empty.appendChild(el('h3', '', 'Composición'));
			empty.appendChild(el('p', 'mini-label', state.analysis_error
				|| (state.assets.length ? 'SIN VALUAR · los activos no tienen valor.' : 'SIN DATOS · cargá un activo para ver la composición.')));
			grid.appendChild(empty);
			return;
		}
		var mixTotal = analysis.mixed_pct > 0
			? 'MIXTOS: ' + pct(analysis.mixed_pct) + ' del total, prorrateado entre RF y RV.'
			: 'SIN ACTIVOS MIXTOS.';
		grid.appendChild(compositionBlock(
			'Tipo de activo',
			'Renta fija vs renta variable, en el orden de la estrategia.',
			analysis.composition.classes,
			{
				kind: 'class',
				target: function (slice) {
					return slice.id === 'fixed_income'
						? state.settings.target_fixed_income_pct
						: state.settings.target_equity_pct;
				},
				note: mixTotal
			}
		));
		grid.appendChild(compositionBlock(
			'Por economía',
			'La economía del subyacente, no la del mercado donde cotiza.',
			analysis.composition.economies,
			{
				isOver: function (slice) {
					return slice.label === analysis.max_economy_label
						&& analysis.max_economy_pct > state.settings.max_economy_pct;
				},
				note: 'La más pesada: ' + analysis.max_economy_label + ' ' + pct(analysis.max_economy_pct)
					+ ' · límite ' + pct(state.settings.max_economy_pct) + '.'
			}
		));
		grid.appendChild(compositionBlock(
			'Por sector',
			'Todos los activos, con la etiqueta de su sector.',
			analysis.composition.sectors,
			{}
		));
		grid.appendChild(compositionBlock(
			'Cíclico vs defensivo',
			'Solo la porción de renta variable.',
			analysis.composition.natures,
			{
				kind: 'nature',
				note: 'Cíclico: tecnología, consumo discrecional, financiero, industrial, materiales e inmobiliario.'
			}
		));
	}

	function findAsset(id) {
		for (var i = 0; i < state.assets.length; i++) {
			if (state.assets[i].id === id) return state.assets[i];
		}
		return null;
	}

	function renderAssets() {
		var body = $('assetsBody');
		body.textContent = '';
		var hasAssets = state.assets.length > 0;
		$('assetsWrap').hidden = !hasAssets;
		$('assetsEmptyNote').hidden = hasAssets;
		if (!hasAssets) return;

		var rows = state.analysis && state.analysis.rows.length ? state.analysis.rows : state.assets.map(function (asset) {
			return {
				id: asset.id,
				ticker: asset.ticker || '',
				name: asset.name,
				class_label: optionLabel(state.options.asset_classes, asset.asset_class),
				class_css: classCss(asset.asset_class),
				economy_label: optionLabel(state.options.economies, asset.economy),
				sector_label: optionLabel(state.options.sectors, asset.sector),
				value_native: asset.market_value,
				currency: asset.currency,
				current_weight_pct: null,
				target_weight_pct: asset.target_weight_pct,
				deviation_pp: null,
				deviation_css: 'neutral',
				n_triggers: asset.triggers.length,
				n_met: asset.triggers.filter(function (t) { return t.is_met; }).length,
				n_pending: asset.triggers.filter(function (t) { return t.is_met && !t.is_resolved; }).length,
				is_cash: asset.is_cash
			};
		});

		for (var i = 0; i < rows.length; i++) {
			(function (row) {
				var asset = findAsset(row.id);
				var tr = el('tr');
				tr.setAttribute('data-asset-id', row.id);

				var assetCell = el('td', 'cell-asset');
				assetCell.appendChild(el('span', 'asset-ticker', row.ticker || row.name));
				if (row.ticker) assetCell.appendChild(el('span', 'asset-name', row.name));
				tr.appendChild(assetCell);

				var classCell = el('td', 'cell-class');
				classCell.setAttribute('data-label', 'CLASE');
				var chipText = row.class_label;
				if (asset && asset.asset_class === 'mixed') {
					chipText += ' ' + chipNum(asset.fixed_income_pct) + '/' + chipNum(100 - asset.fixed_income_pct);
				}
				classCell.appendChild(el('span', 'asset-chip ' + row.class_css, chipText));
				if (row.is_cash) classCell.appendChild(el('span', 'asset-chip cash', 'LIQUIDEZ'));
				tr.appendChild(classCell);

				var exposureCell = el('td', 'cell-exposure');
				exposureCell.setAttribute('data-label', 'EXPOSICIÓN');
				exposureCell.textContent = row.economy_label + ' · ' + row.sector_label;
				tr.appendChild(exposureCell);

				var valueCell = el('td', 'cell-value');
				valueCell.appendChild(el('span', 'asset-value-main',
					fmt(row.value_native, row.currency) + ' ' + row.currency));
				if (row.value_display !== null && row.value_display !== undefined
					&& row.currency !== state.fx.display_currency) {
					valueCell.appendChild(el('span', 'asset-value-sub',
						'= ' + fmt(row.value_display, state.fx.display_currency)
						+ ' ' + state.fx.display_currency));
				}
				tr.appendChild(valueCell);

				var weightCell = el('td', 'cell-weight');
				weightCell.setAttribute('data-label', 'PESO');
				weightCell.textContent = row.current_weight_pct === null ? '—' : pct(row.current_weight_pct);
				tr.appendChild(weightCell);

				var targetCell = el('td', 'cell-target');
				targetCell.setAttribute('data-label', 'OBJETIVO');
				targetCell.textContent = pct(row.target_weight_pct);
				tr.appendChild(targetCell);

				var deviationCell = el('td', 'cell-deviation deviation-' + (row.deviation_css || 'neutral'));
				deviationCell.setAttribute('data-label', 'DESVÍO');
				deviationCell.textContent = row.deviation_pp === null ? '—' : pp(row.deviation_pp);
				tr.appendChild(deviationCell);

				var triggerCell = el('td', 'cell-triggers');
				triggerCell.setAttribute('data-label', 'DISPARADORES');
				triggerCell.textContent = row.n_triggers === 0
					? 'SIN DISPARADORES'
					: row.n_met + '/' + row.n_triggers + ' CUMPLIDOS';
				if (row.n_pending > 0) triggerCell.appendChild(el('span', 'trigger-flag', 'A EJECUTAR'));
				tr.appendChild(triggerCell);

				var actionsCell = el('td', 'cell-actions');
				var actions = el('div', 'row-actions');
				actions.appendChild(button('EDITAR', 'btn btn-mini', function () { editAsset(row.id); }));
				actions.appendChild(button('BORRAR', 'btn btn-mini btn-mini-danger', function () { deleteAsset(row.id); }));
				actions.appendChild(button('DISPARADORES', 'btn btn-mini', function () { viewTriggers(row.id); }));
				actionsCell.appendChild(actions);
				tr.appendChild(actionsCell);

				body.appendChild(tr);
			})(rows[i]);
		}
	}

	function renderRebalance() {
		var list = $('rebalanceList');
		list.textContent = '';
		var analysis = state.analysis;
		var view = $('rebalanceView');
		if (analysis) {
			view.textContent = 'MONTOS EN ' + analysis.display_currency + ' · ' + state.fx.rate_kind
				+ (analysis.rate_used === null ? ' · SIN TASA' : ' ' + nfRate.format(analysis.rate_used));
			view.hidden = false;
		} else {
			view.textContent = '';
			view.hidden = true;
		}
		if (!analysis || analysis.status === 'Sin datos' || analysis.status === 'Sin valuar') {
			list.appendChild(el('div', 'rebalance-empty', analysis && analysis.status === 'Sin valuar'
				? 'SIN VALUAR · los activos no tienen valor.'
				: 'SIN CARTERA · cargá un activo y su peso objetivo para medir.'));
		} else if (!analysis.orders.length) {
			list.appendChild(el('div', 'rebalance-empty',
				'TODO DENTRO DE LA BANDA ±' + num1(state.settings.rebalance_band_pp) + ' pp. No hay órdenes para ejecutar.'));
		} else {
			for (var i = 0; i < analysis.orders.length; i++) {
				(function (order) {
					var row = el('div', 'rebalance-row');
					var action = el('span', 'rebalance-action ' + (order.action === 'Vender' ? 'sell' : 'buy'), order.action);
					row.appendChild(action);
					var detail = el('span', '');
					detail.appendChild(el('span', 'rebalance-asset', order.ticker || order.name));
					if (order.ticker) detail.appendChild(el('span', 'rebalance-detail', order.name));
					detail.appendChild(el('span', 'rebalance-detail', order.detail));
					row.appendChild(detail);
					row.appendChild(el('span', 'rebalance-amount',
						fmt(order.amount, analysis.display_currency) + ' ' + analysis.display_currency));
					row.appendChild(button('EJECUTAR', 'btn btn-mini', function () {
						executeRebalance(order.asset_id);
					}));
					list.appendChild(row);
				})(analysis.orders[i]);
			}
		}

		var drawdown = $('drawdownList');
		drawdown.textContent = '';
		var table = analysis && analysis.drawdown ? analysis.drawdown : [
			{ drop_pct: 10, recovery_pct: 11.1 },
			{ drop_pct: 20, recovery_pct: 25.0 },
			{ drop_pct: 30, recovery_pct: 42.9 },
			{ drop_pct: 40, recovery_pct: 66.7 },
			{ drop_pct: 50, recovery_pct: 100.0 }
		];
		for (var j = 0; j < table.length; j++) {
			drawdown.appendChild(el('div', 'drawdown-item',
				'CAÍDA ' + num1(table[j].drop_pct) + ' % → SUBIR ' + pct(table[j].recovery_pct)));
		}
	}

	async function executeRebalance(assetId) {
		var hasCash = state.assets.some(function (asset) { return asset.is_cash; });
		if (!confirm(hasCash
			? '¿Ejecutar la orden? El activo se ajusta al peso objetivo y la liquidez recibe o paga la diferencia.'
			: '¿Ejecutar la orden? El activo se ajusta al peso objetivo y el total de la cartera se recalcula.')) return;
		var ok = await mutate(
			'/api/portfolio/assets/' + assetId + '/rebalance',
			'POST',
			undefined,
			false,
			'rebalanceStatus'
		);
		if (ok) {
			showSectionNotice('rebalanceStatus', hasCash
				? 'Orden ejecutada · el activo quedó en su objetivo y la liquidez absorbió la diferencia.'
				: 'Orden ejecutada · el activo quedó en su objetivo; el total se recalculó.', 'success');
		}
	}

	function renderTargets() {
		if (targetPreview) cancelTargetPreview();
		var panel = $('targetPanel');
		var analysis = state.analysis;
		if (!analysis || !analysis.targets || analysis.status === 'Sin datos') {
			panel.hidden = true;
			return;
		}
		var targets = analysis.targets;
		var needs = targets.needs_equity || targets.needs_fixed_income;
		panel.hidden = !needs;
		if (!needs) return;
		$('targetSummary').textContent =
			'RV ' + pct(targets.rv_target_sum) + ' / ' + pct(analysis.rv_target_pct)
			+ ' · RF ' + pct(targets.rf_target_sum) + ' / ' + pct(analysis.rf_target_pct)
			+ ' · total ' + pct(targets.total_target_sum);

		var equityBtn = $('targetEquityBtn');
		equityBtn.disabled = !targets.needs_equity || targets.equal_equity_pct === null;
		equityBtn.title = !targets.needs_equity
			? 'La renta variable ya está en su objetivo'
			: (targets.equal_equity_pct === null
				? 'No hay RV puras o la mixta ya ocupa el objetivo'
				: '');

		var fixedBtn = $('targetFixedIncomeBtn');
		fixedBtn.disabled = !targets.needs_fixed_income || targets.equal_fixed_income_pct === null;
		fixedBtn.title = !targets.needs_fixed_income
			? 'La renta fija ya está en su objetivo'
			: (targets.equal_fixed_income_pct === null
				? 'No hay RF puras o la mixta/liquidez ya ocupan el objetivo'
				: '');
	}

	function cancelTargetPreview() {
		targetPreview = null;
		targetPreviewRequest += 1;
		var box = $('targetPreview');
		box.hidden = true;
		box.textContent = '';
	}

	function openTargetPreview(sleeve) {
		targetPreview = { sleeve: sleeve, mode: 'equal', plan: null };
		var box = $('targetPreview');
		box.hidden = false;
		box.textContent = '';
		box.appendChild(el('p', 'mini-label', 'CARGANDO PREVIEW…'));
		fetchTargetPreview();
	}

	async function fetchTargetPreview() {
		if (!targetPreview) return;
		var current = targetPreview;
		var requestId = ++targetPreviewRequest;
		var box = $('targetPreview');
		box.hidden = false;
		box.textContent = '';
		box.appendChild(el('p', 'mini-label', 'CARGANDO PREVIEW…'));
		try {
			var response = await request('/api/portfolio/targets/preview', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ sleeve: current.sleeve, mode: current.mode })
			});
			if (requestId !== targetPreviewRequest || targetPreview !== current) return;
			if (!response.ok) {
				var text = await response.text();
				cancelTargetPreview();
				showSectionNotice('targetStatus', text || 'No se pudo calcular el reparto', 'danger');
				return;
			}
			var plan = await response.json();
			if (requestId !== targetPreviewRequest) return;
			current.plan = plan;
			renderTargetPreview(plan);
		} catch (error) {
			if (requestId !== targetPreviewRequest) return;
			if (error.message !== 'sesión expirada') {
				cancelTargetPreview();
				showSectionNotice('targetStatus', 'No se pudo conectar con el servidor', 'danger');
			}
		}
	}

	function targetModeButton(label, mode, active) {
		var node = button(label, 'target-mode' + (mode === active ? ' is-active' : ''), function () {
			if (!targetPreview || targetPreview.mode === mode) return;
			targetPreview.mode = mode;
			fetchTargetPreview();
		});
		node.setAttribute('aria-pressed', mode === active ? 'true' : 'false');
		return node;
	}

	function renderTargetPreview(plan) {
		var box = $('targetPreview');
		box.textContent = '';

		var head = el('div', 'target-preview-head');
		head.appendChild(el('span', 'mini-label',
			plan.sleeve === 'equity' ? 'REPARTIR RENTA VARIABLE' : 'REPARTIR RENTA FIJA'));
		head.appendChild(el('span', 'mini-label',
			plan.mode === 'equal' ? 'PARTES IGUALES' : 'PROPORCIONES'));
		box.appendChild(head);

		var modes = el('div', 'target-modes');
		modes.appendChild(targetModeButton('PARTES IGUALES', 'equal', plan.mode));
		modes.appendChild(targetModeButton('MANTENER PROPORCIONES', 'proportional', plan.mode));
		box.appendChild(modes);

		var table = el('div', 'target-preview-table');
		var header = el('div', 'target-preview-row target-preview-row-head');
		header.appendChild(el('span', 'mini-label', 'ACTIVO'));
		header.appendChild(el('span', 'mini-label', 'ACTUAL'));
		header.appendChild(el('span', 'mini-label', 'PROPUESTO'));
		header.appendChild(el('span', 'mini-label', 'DIF.'));
		table.appendChild(header);
		for (var i = 0; i < plan.changes.length; i++) {
			var change = plan.changes[i];
			var row = el('div', 'target-preview-row');
			var asset = el('span', 'target-preview-asset');
			asset.appendChild(el('span', 'target-preview-ticker', change.ticker || change.name));
			if (change.ticker) asset.appendChild(el('span', 'target-preview-name', change.name));
			row.appendChild(asset);
			row.appendChild(el('span', 'target-preview-num', pct(change.old_target_pct)));
			row.appendChild(el('span', 'target-preview-num', pct(change.new_target_pct)));
			var deltaClass = 'target-preview-num';
			if (change.delta_pct > 0.005) deltaClass += ' up';
			else if (change.delta_pct < -0.005) deltaClass += ' down';
			row.appendChild(el('span', deltaClass, pp(change.delta_pct)));
			table.appendChild(row);
		}
		box.appendChild(table);

		var summary = el('div', 'target-preview-summary');
		summary.appendChild(el('span', 'mini-label', 'RESULTADO'));
		summary.appendChild(el('span', 'target-preview-summary-text',
			'RV ' + pct(plan.rv_target_before) + ' → ' + pct(plan.rv_target_after)
			+ ' · RF ' + pct(plan.rf_target_before) + ' → ' + pct(plan.rf_target_after)
			+ ' · total ' + pct(plan.total_before) + ' → ' + pct(plan.total_after)));
		box.appendChild(summary);

		for (var j = 0; j < plan.warnings.length; j++) {
			box.appendChild(el('p', 'target-warning', plan.warnings[j]));
		}

		var actions = el('div', 'target-preview-actions');
		var apply = button('APLICAR', 'btn btn-primary', applyTargetPreview);
		apply.disabled = !plan.changes.some(function (change) {
			return Math.abs(change.delta_pct) >= 0.005;
		});
		actions.appendChild(apply);
		actions.appendChild(button('CANCELAR', 'btn', cancelTargetPreview));
		box.appendChild(actions);
	}

	async function applyTargetPreview() {
		if (!targetPreview) return;
		var plan = targetPreview.plan;
		var body = { sleeve: targetPreview.sleeve, mode: targetPreview.mode };
		var ok = await mutate('/api/portfolio/targets/apply', 'POST', body, false, 'targetStatus');
		if (ok) {
			targetPreview = null;
			if (plan && Math.abs(plan.total_after - 100) > 0.05) {
				showSectionNotice('targetStatus',
					'Objetivos actualizados · el total queda en ' + pct(plan.total_after)
					+ '; falta ajustar el otro bloque.');
			} else {
				showSectionNotice('targetStatus', 'Objetivos actualizados · no se movió dinero.', 'success');
			}
		}
	}

	function pendingTriggerList() {
		if (state.analysis && state.analysis.pending_triggers) return state.analysis.pending_triggers;
		var list = [];
		for (var i = 0; i < state.assets.length; i++) {
			var asset = state.assets[i];
			for (var j = 0; j < asset.triggers.length; j++) {
				var trigger = asset.triggers[j];
				if (trigger.is_met && !trigger.is_resolved) {
					list.push({
						id: trigger.id,
						asset_id: asset.id,
						asset: asset.ticker || asset.name,
						kind: optionLabel(state.options.trigger_kinds, trigger.kind),
						description: trigger.description
					});
				}
			}
		}
		return list;
	}

	function renderPendingTriggers() {
		var node = $('pendingTriggers');
		node.textContent = '';
		var list = pendingTriggerList();
		if (!list.length) {
			node.hidden = true;
			return;
		}
		node.hidden = false;
		node.appendChild(el('h4', '', 'A EJECUTAR'));
		node.appendChild(el('p', 'mini-label pending-note',
			'DISPARADORES CUMPLIDOS DE TODOS LOS ACTIVOS · LA EJECUCIÓN VENDE EL ACTIVO COMPLETO.'));
		for (var i = 0; i < list.length; i++) {
			(function (pending) {
				var item = el('div', 'pending-item');
				var head = el('div', '');
				head.appendChild(el('b', '', pending.asset));
				head.appendChild(el('span', '', ' · ' + pending.kind));
				item.appendChild(head);
				item.appendChild(el('p', '', pending.description));
				var actions = el('div', 'pending-actions');
				actions.appendChild(button('VER ACTIVO', 'btn btn-mini', function () {
					openTriggerAsset(pending.asset_id);
				}));
				actions.appendChild(button('MARCAR EJECUTADO', 'btn btn-mini btn-mini-danger', function () {
					executeTrigger(pending.id);
				}));
				item.appendChild(actions);
				node.appendChild(item);
			})(list[i]);
		}
	}

	function openTriggerAsset(assetId) {
		selectedTriggerAssetId = assetId;
		renderTriggers();
		var panel = $('triggerPanel');
		if (panel) panel.scrollIntoView({ block: 'start' });
	}

	async function executeTrigger(triggerId) {
		var hasCash = state.assets.some(function (asset) { return asset.is_cash; });
		if (!confirm(hasCash
			? '¿Marcar como ejecutado? Se vende todo el activo, la liquidez recibe la venta y el disparador queda cerrado.'
			: '¿Marcar como ejecutado? Se vende todo el activo, sale de la cartera y el disparador queda cerrado.')) return;
		var ok = await mutate(
			'/api/portfolio/triggers/' + triggerId + '/execute',
			'POST',
			undefined,
			false,
			'triggerStatus'
		);
		if (ok) {
			showSectionNotice('triggerStatus', hasCash
				? 'Disparador ejecutado · el activo salió de la cartera y la liquidez recibió la venta.'
				: 'Disparador ejecutado · el activo salió de la cartera; el total se recalculó.', 'success');
		}
	}

	async function undoExecution(triggerId) {
		var ok = await mutate(
			'/api/portfolio/triggers/' + triggerId,
			'PATCH',
			{ is_resolved: false },
			false,
			'triggerStatus'
		);
		if (ok) showSectionNotice('triggerStatus', 'Ejecución deshecha', 'success');
	}

	function renderTriggers() {
		renderPendingTriggers();
		var select = $('triggerAsset');
		var previous = selectedTriggerAssetId || select.value;
		select.textContent = '';
		if (!state.assets.length) {
			select.appendChild(el('option', '', 'SIN ACTIVOS'));
			$('triggerList').textContent = '';
			$('triggerList').appendChild(el('p', 'trigger-empty', 'Agregá un activo para escribirle sus disparadores de venta.'));
			$('triggerSummary').textContent = '';
			$('triggerKind').disabled = true;
			$('triggerDescription').disabled = true;
			$('triggerSubmit').disabled = true;
			$('triggerHelp').textContent = 'AGREGÁ UN ACTIVO PRIMERO.';
			return;
		}

		var selected = null;
		for (var i = 0; i < state.assets.length; i++) {
			var asset = state.assets[i];
			var option = document.createElement('option');
			option.value = asset.id;
			option.textContent = (asset.ticker ? asset.ticker + ' · ' : '') + asset.name;
			select.appendChild(option);
			if (asset.id === previous) selected = asset;
		}
		if (!selected) {
			selected = state.assets[0];
			selectedTriggerAssetId = selected.id;
		}
		select.value = selected.id;

		var cashSelected = !!selected.is_cash;
		$('triggerKind').disabled = cashSelected;
		$('triggerDescription').disabled = cashSelected;
		$('triggerSubmit').disabled = cashSelected;
		$('triggerHelp').textContent = cashSelected
			? 'LA LIQUIDEZ ES LA CONTRAPARTE DEL REBALANCEO; NO LLEVA DISPARADORES.'
			: 'Un disparador es una condición escrita: si se cumple, se vende; si no, no.';

		var list = $('triggerList');
		list.textContent = '';
		var pending = 0;
		var resolved = 0;
		if (!selected.triggers.length) {
			list.appendChild(el('p', 'trigger-empty', cashSelected
				? 'LA LIQUIDEZ ES LA CONTRAPARTE DEL REBALANCEO; NO LLEVA DISPARADORES.'
				: 'SIN DISPARADORES · el activo entró sin plan de salida. Escribí al menos uno.'));
		} else {
			for (var j = 0; j < selected.triggers.length; j++) {
				(function (trigger) {
					var isPending = trigger.is_met && !trigger.is_resolved;
					if (isPending) pending++;
					if (trigger.is_resolved) resolved++;
					var item = el('div', 'trigger-item'
						+ (isPending ? ' is-met' : '')
						+ (trigger.is_resolved ? ' is-resolved' : ''));
					var body = el('div', '');
					body.appendChild(el('span', 'trigger-kind',
						optionLabel(state.options.trigger_kinds, trigger.kind)));
					if (trigger.is_resolved) body.appendChild(el('span', 'trigger-chip', 'EJECUTADO'));
					body.appendChild(el('p', 'trigger-description', trigger.description));
					item.appendChild(body);
					var actions = el('div', 'trigger-actions');
					if (trigger.is_resolved) {
						actions.appendChild(button('DESHACER', 'btn btn-mini', function () {
							undoExecution(trigger.id);
						}));
					} else if (trigger.is_met) {
						actions.appendChild(button('MARCAR EJECUTADO', 'btn btn-mini btn-mini-danger', function () {
							executeTrigger(trigger.id);
						}));
						actions.appendChild(button('REABRIR', 'btn btn-mini', function () {
							toggleTrigger(trigger.id, false);
						}));
					} else {
						actions.appendChild(button('MARCAR CUMPLIDO', 'btn btn-mini', function () {
							toggleTrigger(trigger.id, true);
						}));
					}
					actions.appendChild(button('BORRAR', 'btn btn-mini btn-mini-danger', function () {
						removeTrigger(trigger.id);
					}));
					item.appendChild(actions);
					list.appendChild(item);
				})(selected.triggers[j]);
			}
		}
		$('triggerSummary').textContent = cashSelected
			? 'LIQUIDEZ · CONTRAPARTE'
			: (selected.triggers.length
				? selected.triggers.length + ' DISPARADORES · ' + pending + ' A EJECUTAR'
					+ (resolved ? ' · ' + resolved + ' EJECUTADOS' : '')
				: 'SIN DISPARADORES');
	}

	function renderSettingsValues() {
		var settings = state.settings;
		$('settingsProfile').value = settings.risk_profile;
		$('settingsValuation').value = settings.valuation;
		$('settingsFixed').value = settings.target_fixed_income_pct;
		$('settingsEquity').value = settings.target_equity_pct;
		$('settingsBand').value = settings.rebalance_band_pp;
		$('settingsMin').value = settings.min_assets;
		$('settingsMax').value = settings.max_assets;
		$('settingsMaxAsset').value = settings.max_asset_pct;
		$('settingsMaxEconomy').value = settings.max_economy_pct;
		$('settingsMaxCluster').value = settings.max_cluster_pct;
		$('settingsCustom').hidden = !state.is_custom;
	}

	function formMatchesProfile() {
		var selected = $('settingsProfile').value;
		var preset = null;
		for (var i = 0; i < state.profiles.length; i++) {
			if (state.profiles[i].id === selected) preset = state.profiles[i];
		}
		if (!preset) return false;
		var eps = 0.011;
		return Math.abs(parseFloat($('settingsFixed').value) - preset.target_fixed_income_pct) <= eps
			&& Math.abs(parseFloat($('settingsEquity').value) - preset.target_equity_pct) <= eps
			&& Math.abs(parseFloat($('settingsBand').value) - preset.rebalance_band_pp) <= eps
			&& parseInt($('settingsMin').value, 10) === preset.min_assets
			&& parseInt($('settingsMax').value, 10) === preset.max_assets
			&& Math.abs(parseFloat($('settingsMaxAsset').value) - preset.max_asset_pct) <= eps
			&& Math.abs(parseFloat($('settingsMaxEconomy').value) - preset.max_economy_pct) <= eps
			&& Math.abs(parseFloat($('settingsMaxCluster').value) - preset.max_cluster_pct) <= eps;
	}

	function markSettingsCustom() {
		$('settingsCustom').hidden = formMatchesProfile();
	}

	function applyProfile(id) {
		var preset = null;
		for (var i = 0; i < state.profiles.length; i++) {
			if (state.profiles[i].id === id) preset = state.profiles[i];
		}
		if (!preset) return;
		$('settingsProfile').value = preset.id;
		$('settingsFixed').value = preset.target_fixed_income_pct;
		$('settingsEquity').value = preset.target_equity_pct;
		$('settingsBand').value = preset.rebalance_band_pp;
		$('settingsMin').value = preset.min_assets;
		$('settingsMax').value = preset.max_assets;
		$('settingsMaxAsset').value = preset.max_asset_pct;
		$('settingsMaxEconomy').value = preset.max_economy_pct;
		$('settingsMaxCluster').value = preset.max_cluster_pct;
		$('settingsCustom').hidden = true;
		var buttons = $('profileSwitch').querySelectorAll('.profile-btn');
		for (var j = 0; j < buttons.length; j++) {
			buttons[j].setAttribute('aria-pressed', buttons[j].getAttribute('data-profile') === id ? 'true' : 'false');
		}
		showSectionNotice('settingsStatus', 'Perfil ' + preset.label + ' aplicado · guardá los ajustes para confirmar', 'success');
	}

	async function saveSettings() {
		var fixed = parseFloat($('settingsFixed').value);
		var equity = parseFloat($('settingsEquity').value);
		var band = parseFloat($('settingsBand').value);
		var minAssets = parseInt($('settingsMin').value, 10);
		var maxAssets = parseInt($('settingsMax').value, 10);
		var maxAsset = parseFloat($('settingsMaxAsset').value);
		var maxEconomy = parseFloat($('settingsMaxEconomy').value);
		var maxCluster = parseFloat($('settingsMaxCluster').value);

		if (![fixed, equity, band, maxAsset, maxEconomy, maxCluster].every(isFinite)
			|| !isFinite(minAssets) || !isFinite(maxAssets)) {
			showSectionNotice('settingsStatus', 'Revisá los números de los ajustes', 'danger');
			return false;
		}
		if (Math.abs(fixed + equity - 100) > 0.01) {
			showSectionNotice('settingsStatus', 'Los objetivos de renta fija y variable deben sumar 100 %', 'danger');
			return false;
		}
		if (minAssets < 0 || maxAssets < minAssets) {
			showSectionNotice('settingsStatus', 'El rango de activos no es válido', 'danger');
			return false;
		}

		var payload = {
			risk_profile: $('settingsProfile').value,
			valuation: $('settingsValuation').value,
			target_fixed_income_pct: fixed,
			target_equity_pct: equity,
			rebalance_band_pp: band,
			min_assets: minAssets,
			max_assets: maxAssets,
			max_asset_pct: maxAsset,
			max_economy_pct: maxEconomy,
			max_cluster_pct: maxCluster
		};
		var ok = await mutate('/api/portfolio/settings', 'PUT', payload, true, 'settingsStatus');
		if (ok) showSectionNotice('settingsStatus', 'Ajustes guardados', 'success');
		else renderFx();
		return ok;
	}

	function resetAssetForm() {
		editingAssetId = null;
		$('assetForm').reset();
		$('assetClass').value = 'equity';
		$('assetEconomy').value = 'us';
		$('assetSector').value = 'technology';
		$('assetCurrency').value = 'USD';
		$('assetTarget').value = '0';
		$('assetFixedIncomePct').value = '50';
		$('assetFixedIncomePct').disabled = true;
		$('assetFixedIncomeWrap').hidden = true;
		$('assetIsCash').checked = false;
		$('assetIsCash').disabled = true;
		$('assetFormTitle').textContent = 'AGREGAR ACTIVO';
		$('assetFormSubmit').textContent = 'AGREGAR';
		$('assetFormCancel').hidden = true;
		$('assetStatus').hidden = true;
	}

	function editAsset(id) {
		var asset = findAsset(id);
		if (!asset) return;
		editingAssetId = id;
		$('assetStatus').hidden = true;
		$('assetTicker').value = asset.ticker || '';
		$('assetName').value = asset.name;
		$('assetClass').value = asset.asset_class;
		$('assetFixedIncomePct').value = asset.fixed_income_pct;
		$('assetEconomy').value = asset.economy;
		$('assetSector').value = asset.sector;
		$('assetValue').value = asset.market_value;
		$('assetCurrency').value = asset.currency;
		$('assetTarget').value = asset.target_weight_pct;
		$('assetThesis').value = asset.thesis || '';
		$('assetObjective').value = asset.objective || '';
		var isMixed = asset.asset_class === 'mixed';
		var isFixedIncome = asset.asset_class === 'fixed_income';
		$('assetFixedIncomePct').value = isMixed ? asset.fixed_income_pct : '50';
		$('assetFixedIncomePct').disabled = !isMixed;
		$('assetFixedIncomeWrap').hidden = !isMixed;
		$('assetIsCash').disabled = !isFixedIncome;
		$('assetIsCash').checked = !!asset.is_cash;
		$('assetFormTitle').textContent = 'EDITAR ACTIVO';
		$('assetFormSubmit').textContent = 'GUARDAR';
		$('assetFormCancel').hidden = false;
		$('sec-assets').scrollIntoView({ block: 'start' });
		$('assetName').focus();
	}

	async function deleteAsset(id) {
		if (!confirm('¿Borrar el activo y todos sus disparadores?')) return;
		var ok = await mutate('/api/portfolio/assets/' + id, 'DELETE', undefined, false, 'assetStatus');
		if (ok) {
			if (selectedTriggerAssetId === id) selectedTriggerAssetId = null;
			if (editingAssetId === id) resetAssetForm();
			showSectionNotice('assetStatus', 'Activo borrado', 'success');
		}
	}

	async function clearPortfolio() {
		if (!confirm('¿Vaciar la cartera? Se borran todos los activos y sus disparadores. No se tocan tus cuentas ni tu libro.')) return;
		var ok = await mutate('/api/portfolio/assets', 'DELETE', undefined, false, 'assetStatus');
		if (ok) {
			selectedTriggerAssetId = null;
			resetAssetForm();
			showSectionNotice('assetStatus', 'Cartera vaciada · podés cargar el ejemplo o empezar de cero.', 'success');
		}
	}

	function viewTriggers(id) {
		selectedTriggerAssetId = id;
		renderTriggers();
		$('sec-triggers').scrollIntoView({ block: 'start' });
	}

	async function toggleTrigger(id, isMet) {
		var ok = await mutate(
			'/api/portfolio/triggers/' + id,
			'PATCH',
			{ is_met: isMet },
			false,
			'triggerStatus'
		);
		if (ok) showSectionNotice('triggerStatus',
			isMet ? 'Disparador marcado como cumplido' : 'Disparador reabierto', 'success');
	}

	async function removeTrigger(id) {
		if (!confirm('¿Borrar este disparador?')) return;
		var ok = await mutate('/api/portfolio/triggers/' + id, 'DELETE', undefined, false, 'triggerStatus');
		if (ok) showSectionNotice('triggerStatus', 'Disparador borrado', 'success');
	}

	async function submitAsset(event) {
		event.preventDefault();
		var name = $('assetName').value.trim();
		var value = parseFloat($('assetValue').value);
		var target = parseFloat($('assetTarget').value);
		var assetClass = $('assetClass').value;
		var fixedPart = parseFloat($('assetFixedIncomePct').value);

		if (!name) {
			showSectionNotice('assetStatus', 'El nombre del activo es obligatorio', 'danger');
			return;
		}
		if (!isFinite(value) || value < 0) {
			showSectionNotice('assetStatus', 'El valor del activo no es válido', 'danger');
			return;
		}
		if (!isFinite(target) || target < 0 || target > 100) {
			showSectionNotice('assetStatus', 'El peso objetivo debe estar entre 0 y 100 %', 'danger');
			return;
		}
		if (assetClass === 'mixed' && (!isFinite(fixedPart) || fixedPart <= 0 || fixedPart >= 100)) {
			showSectionNotice('assetStatus', 'La parte de renta fija debe estar entre 1 y 99 %', 'danger');
			return;
		}

		var payload = {
			ticker: $('assetTicker').value.trim() || null,
			name: name,
			asset_class: assetClass,
			fixed_income_pct: assetClass === 'mixed' ? fixedPart : null,
			economy: $('assetEconomy').value,
			sector: $('assetSector').value,
			market_value: value,
			currency: $('assetCurrency').value,
			target_weight_pct: target,
			thesis: $('assetThesis').value.trim(),
			objective: $('assetObjective').value.trim(),
			is_cash: $('assetIsCash').checked
		};

		var editing = editingAssetId !== null;
		var url = editing ? '/api/portfolio/assets/' + editingAssetId : '/api/portfolio/assets';
		var ok = await mutate(url, editing ? 'PATCH' : 'POST', payload, false, 'assetStatus');
		if (ok) {
			resetAssetForm();
			showSectionNotice('assetStatus',
				editing ? 'Activo guardado · el rebalanceo se recalculó' : 'Activo agregado', 'success');
		}
	}

	async function loadExample() {
		if (!confirm('¿Cargar la cartera de ejemplo? Se agregan 13 activos y sus disparadores.')) return;
		var ok = await mutate('/api/portfolio/example', 'POST', undefined, false);
		if (ok) showNotice('Cartera de ejemplo cargada', 'success');
	}

	async function refreshRates() {
		var btn = $('refreshRates');
		btn.disabled = true;
		try {
			var response = await request('/api/portfolio/refresh', { method: 'POST' });
			if (!response.ok) {
				showNotice(await response.text() || 'No se pudieron actualizar las tasas', 'danger');
				return;
			}
			var next = await response.json();
			applyState(next, false);
			if (state.fx.error) {
				showNotice('No se pudieron leer las tasas: ' + state.fx.error, 'danger');
			} else if (state.fx.stale) {
				showNotice('Se mantiene la última tasa disponible', 'success');
			} else {
				showNotice('Tasas actualizadas', 'success');
			}
		} catch (error) {
			if (error.message !== 'sesión expirada') {
				showNotice('No se pudo conectar con el servidor', 'danger');
			}
		} finally {
			btn.disabled = false;
		}
	}

	async function load() {
		try {
			var response = await request('/api/portfolio');
			if (!response.ok) {
				showNotice(await response.text() || 'No se pudo cargar la cartera', 'danger');
				return;
			}
			applyState(await response.json(), true);
		} catch (error) {
			if (error.message !== 'sesión expirada') {
				showNotice('No se pudo conectar con el servidor', 'danger');
			}
		}
	}

	// ---------- eventos ----------

	$('logoutBtn').addEventListener('click', async function () {
		await fetch('/api/logout', { method: 'POST' });
		window.location.href = '/login';
	});

	$('refreshRates').addEventListener('click', refreshRates);

	$('targetEquityBtn').addEventListener('click', function () {
		openTargetPreview('equity');
	});

	$('targetFixedIncomeBtn').addEventListener('click', function () {
		openTargetPreview('fixed_income');
	});

	$('valuationSelect').addEventListener('change', function () {
		$('settingsValuation').value = this.value;
		saveSettings();
	});

	$('assetClass').addEventListener('change', function () {
		var mixed = this.value === 'mixed';
		var fixedIncome = this.value === 'fixed_income';
		var split = $('assetFixedIncomePct');
		$('assetFixedIncomeWrap').hidden = !mixed;
		split.disabled = !mixed;
		if (mixed) {
			var value = parseFloat(split.value);
			if (!isFinite(value) || value < 1 || value > 99) split.value = '50';
		}
		$('assetIsCash').disabled = !fixedIncome;
		if (!fixedIncome) $('assetIsCash').checked = false;
	});

	$('assetForm').addEventListener('submit', submitAsset);
	$('assetFormCancel').addEventListener('click', resetAssetForm);
	$('clearPortfolio').addEventListener('click', clearPortfolio);

	$('emptyAddAsset').addEventListener('click', function () {
		$('sec-assets').scrollIntoView({ block: 'start' });
		$('assetName').focus();
	});

	$('emptyExample').addEventListener('click', loadExample);

	$('triggerAsset').addEventListener('change', function () {
		selectedTriggerAssetId = this.value;
		renderTriggers();
	});

	$('triggerForm').addEventListener('submit', async function (event) {
		event.preventDefault();
		if (!selectedTriggerAssetId) {
			showSectionNotice('triggerStatus', 'Elegí un activo', 'danger');
			return;
		}
		var description = $('triggerDescription').value.trim();
		if (!description) {
			showSectionNotice('triggerStatus', 'La condición del disparador es obligatoria', 'danger');
			return;
		}
		var ok = await mutate(
			'/api/portfolio/assets/' + selectedTriggerAssetId + '/triggers',
			'POST',
			{ kind: $('triggerKind').value, description: description },
			false,
			'triggerStatus'
		);
		if (ok) {
			$('triggerDescription').value = '';
			showSectionNotice('triggerStatus', 'Disparador agregado', 'success');
		}
	});

	$('settingsForm').addEventListener('submit', function (event) {
		event.preventDefault();
		saveSettings();
	});

	var settingsInputs = $('settingsForm').querySelectorAll('input, select');
	for (var i = 0; i < settingsInputs.length; i++) {
		settingsInputs[i].addEventListener('input', markSettingsCustom);
		settingsInputs[i].addEventListener('change', markSettingsCustom);
	}

	window.addEventListener('resize', function () {
		if (state && state.analysis) fitTotal($('heroTotal'));
	});

	load();
})();
