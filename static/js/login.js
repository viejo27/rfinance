document.getElementById('loginForm').addEventListener('submit', function(e) {
	e.preventDefault();
	
	const email = document.getElementById('email').value;
	const password = document.getElementById('password').value;
	
	const validEmail = 'admin@admin.com';
	const validPassword = 'password';
	
	if (email === validEmail && password === validPassword) {
		alert('Login exitoso!');
		window.location.href = '/';
	} else {
		alert('Credenciales incorrectas');
		document.getElementById('password').value = '';
	}
});
