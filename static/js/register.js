const notice = document.getElementById('registerMsg');

function show(text, isError) {
	notice.textContent = text;
	notice.classList.toggle('notice-danger', isError);
	notice.hidden = false;
}

document.getElementById('registerForm').addEventListener('submit', async function (e) {
	e.preventDefault();
	notice.hidden = true;

	const email = document.getElementById('email').value.trim();
	const password = document.getElementById('password').value.trim();
	const name = document.getElementById('name').value.trim();
	const last_name = document.getElementById('last_name').value.trim();

	if (!email || !password || !name || !last_name) {
		show('Todos los campos son obligatorios — complétalos para continuar.', true);
		return;
	}

	// Espejo del server (register.rs): la API es pública y el server
	// revalida, acá es solo para dar el error antes.
	if (password.length < 10 || password.length > 72) {
		show('La contraseña debe tener entre 10 y 72 caracteres.', true);
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
			show('Registro exitoso — puedes iniciar sesión.', false);
			setTimeout(function () {
				window.location.href = '/login';
			}, 900);
		} else {
			const errorText = await response.text();
			show('No se pudo registrar: ' + errorText, true);
		}
	} catch (error) {
		show('Error de conexión con el servidor — inténtalo de nuevo.', true);
	}
});
