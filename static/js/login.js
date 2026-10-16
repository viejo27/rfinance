const notice = document.getElementById('loginError');

function showError(text) {
	notice.textContent = text;
	notice.hidden = false;
}

document.getElementById('loginForm').addEventListener('submit', async function (e) {
	e.preventDefault();
	notice.hidden = true;

	const email = document.getElementById('email').value;
	const password = document.getElementById('password').value;

	const response = await fetch('/api/login', {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json'
		},
		body: JSON.stringify({ email, password })
	});

	if (response.ok) {
		window.location.href = '/';
	} else {
		showError('Credenciales incorrectas — revisa el email y la contraseña, o regístrate.');
		document.getElementById('password').value = '';
		document.getElementById('email').focus();
	}
});
