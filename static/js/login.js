document.getElementById('loginForm').addEventListener('submit', async function(e) {
	e.preventDefault();
	
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
		alert('Credenciales incorrectas');
		document.getElementById('password').value = '';
	}
});
