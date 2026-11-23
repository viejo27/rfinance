const notice = document.getElementById('forgotMsg');
const form = document.getElementById('forgotForm');
const submit = form.querySelector('button[type="submit"]');

function show(text, isError) {
	notice.textContent = text;
	notice.classList.toggle('notice-danger', isError);
	notice.classList.toggle('notice-success', !isError);
	notice.hidden = false;
}

form.addEventListener('submit', async function (e) {
	e.preventDefault();
	notice.hidden = true;
	submit.disabled = true;

	const email = document.getElementById('email').value.trim();

	try {
		const response = await fetch('/api/forgot-password', {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ email })
		});
		const text = await response.text();
		show(text, !response.ok);
	} catch (error) {
		show('Error de conexión con el servidor — inténtalo de nuevo.', true);
	} finally {
		submit.disabled = false;
	}
});
