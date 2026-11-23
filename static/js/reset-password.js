const notice = document.getElementById('resetMsg');
const form = document.getElementById('resetForm');

function show(text, isError) {
	notice.textContent = text;
	notice.classList.toggle('notice-danger', isError);
	notice.classList.toggle('notice-success', !isError);
	notice.hidden = false;
}

if (form) {
	const submit = form.querySelector('button[type="submit"]');
	const token = form.dataset.token;

	form.addEventListener('submit', async function (e) {
		e.preventDefault();
		notice.hidden = true;

		const password = document.getElementById('password').value;
		const confirm = document.getElementById('passwordConfirm').value;

		if (password.length < 10 || password.length > 72) {
			show('La contraseña debe tener entre 10 y 72 caracteres.', true);
			return;
		}
		if (password !== confirm) {
			show('Las contraseñas no coinciden.', true);
			return;
		}

		submit.disabled = true;
		try {
			const response = await fetch('/api/reset-password', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ token, password })
			});
			const text = await response.text();
			show(text, !response.ok);
			if (response.ok) {
				setTimeout(function () {
					window.location.href = '/login';
				}, 1200);
			} else {
				submit.disabled = false;
			}
		} catch (error) {
			show('Error de conexión con el servidor — inténtalo de nuevo.', true);
			submit.disabled = false;
		}
	});
}
