const aviso = document.getElementById('registerMsg');

function mostrar(texto, esError) {
	aviso.textContent = texto;
	aviso.classList.toggle('aviso-error', esError);
	aviso.hidden = false;
}

document.getElementById('registerForm').addEventListener('submit', async function (e) {
	e.preventDefault();
	aviso.hidden = true;

	const email = document.getElementById('email').value.trim();
	const password = document.getElementById('password').value.trim();
	const name = document.getElementById('name').value.trim();
	const last_name = document.getElementById('last_name').value.trim();

	if (!email || !password || !name || !last_name) {
		mostrar('Todos los campos son obligatorios — complétalos para continuar.', true);
		return;
	}

	try {
		const response = await fetch('/api/register', {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({ email, password, name, last_name })
		});

		if (response.ok) {
			mostrar('Registro exitoso — puedes iniciar sesión.', false);
			setTimeout(function () {
				window.location.href = '/login';
			}, 900);
		} else {
			const errorText = await response.text();
			mostrar('No se pudo registrar: ' + errorText, true);
		}
	} catch (error) {
		mostrar('Error de conexión con el servidor — inténtalo de nuevo.', true);
	}
});
