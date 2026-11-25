// RFinance — diálogo de confirmación compartido.
// Reemplaza window.confirm con un <dialog> nativo vestido como la prensa:
// hoja de papel, banda roja recortada y botones del mundo.
// Uso: RFinance.confirm({ title, message, confirmLabel, cancelLabel, danger })
// Devuelve Promise<boolean>: true solo si se confirma la acción.

(function () {
	'use strict';

	var dialog = null;
	var titleNode = null;
	var messageNode = null;
	var confirmButton = null;
	var cancelButton = null;
	var pending = null;
	var closing = false;
	var restoreFocus = null;

	function build() {
		dialog = document.createElement('dialog');
		dialog.className = 'app-dialog';
		dialog.setAttribute('role', 'alertdialog');
		dialog.setAttribute('aria-labelledby', 'appDialogTitle');
		dialog.setAttribute('aria-describedby', 'appDialogMessage');

		var band = document.createElement('header');
		band.className = 'dialog-band';

		titleNode = document.createElement('h2');
		titleNode.className = 'dialog-title';
		titleNode.id = 'appDialogTitle';
		band.appendChild(titleNode);

		var body = document.createElement('div');
		body.className = 'dialog-body';

		messageNode = document.createElement('p');
		messageNode.className = 'dialog-message';
		messageNode.id = 'appDialogMessage';
		body.appendChild(messageNode);

		var actions = document.createElement('div');
		actions.className = 'dialog-actions';

		cancelButton = document.createElement('button');
		cancelButton.type = 'button';
		cancelButton.className = 'btn dialog-cancel';
		cancelButton.addEventListener('click', function () {
			close('cancel');
		});

		confirmButton = document.createElement('button');
		confirmButton.type = 'button';
		confirmButton.className = 'btn dialog-confirm';
		confirmButton.addEventListener('click', function () {
			close('confirm');
		});

		actions.appendChild(cancelButton);
		actions.appendChild(confirmButton);
		body.appendChild(actions);

		dialog.appendChild(band);
		dialog.appendChild(body);
		document.body.appendChild(dialog);

		dialog.addEventListener('cancel', function (event) {
			event.preventDefault();
			close('cancel');
		});

		// El clic fuera de la hoja cae sobre el propio <dialog>: cancela.
		dialog.addEventListener('click', function (event) {
			if (event.target === dialog) close('cancel');
		});

		dialog.addEventListener('close', function () {
			closing = false;
			var request = pending;
			pending = null;
			if (request) request.resolve(dialog.returnValue === 'confirm');
			if (restoreFocus && restoreFocus !== document.body && document.contains(restoreFocus)) {
				restoreFocus.focus();
			}
			restoreFocus = null;
		});
	}

	function close(value) {
		if (!dialog || !dialog.open) return;
		closing = true;
		dialog.close(value);
	}

	function supportsDialog() {
		var probe = document.createElement('dialog');
		return typeof probe.showModal === 'function';
	}

	function ask(options) {
		options = options || {};

		var fallback = [options.title, options.message].filter(Boolean).join('\n\n');

		if (!dialog) {
			// Sin soporte de <dialog>, no se crea un elemento que quedaría visible.
			if (!supportsDialog()) return Promise.resolve(window.confirm(fallback));
			build();
		}
		if (typeof dialog.showModal !== 'function') {
			return Promise.resolve(window.confirm(fallback));
		}
		// Un modal hace inerte la página; otra llamada mientras tanto no apila.
		if (dialog.open || closing) return Promise.resolve(false);

		titleNode.textContent = options.title || 'CONFIRMAR';
		messageNode.textContent = options.message || '';
		cancelButton.textContent = options.cancelLabel || 'CANCELAR';
		confirmButton.textContent = options.confirmLabel || 'CONFIRMAR';
		confirmButton.classList.toggle('btn-primary', !options.danger);
		confirmButton.classList.toggle('btn-danger', !!options.danger);
		dialog.returnValue = '';
		restoreFocus = document.activeElement;

		return new Promise(function (resolve) {
			pending = { resolve: resolve };
			try {
				dialog.showModal();
			} catch (error) {
				pending = null;
				resolve(window.confirm(fallback));
				return;
			}
			// La acción destructiva arranca en CANCELAR; la primaria, en confirmar.
			(options.danger ? cancelButton : confirmButton).focus();
		});
	}

	window.RFinance = window.RFinance || {};
	window.RFinance.confirm = ask;
})();
