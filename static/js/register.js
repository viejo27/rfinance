document.getElementById('registerForm').addEventListener('submit', async function(e) {
    e.preventDefault();

    const email = document.getElementById('email').value.trim();
    const password = document.getElementById('password').value.trim();
    const name = document.getElementById('name').value.trim();
    const last_name = document.getElementById('last_name').value.trim();

    if (!email || !password || !name || !last_name) {
        alert('Todos los campos son obligatorios');
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
            alert('Registro exitoso!');
            window.location.href = '/login';
        } else {
            const errorText = await response.text();
            alert('Error: ' + errorText);
        }
    } catch (error) {
        alert('Error de conexión: ' + error.message);
    }
});
