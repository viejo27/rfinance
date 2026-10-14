const aviso = document.getElementById('loginError');

function mostrarError(texto) {
	aviso.textContent = texto;
	aviso.hidden = false;
}

document.getElementById('loginForm').addEventListener('submit', async function (e) {
	e.preventDefault();
	aviso.hidden = true;

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
		mostrarError('Credenciales incorrectas — revisa el email y la contraseña, o regístrate.');
		document.getElementById('password').value = '';
		document.getElementById('email').focus();
	}
});
