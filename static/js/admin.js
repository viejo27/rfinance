document.getElementById('logoutBtn').addEventListener('click', async function () {
	await fetch('/api/logout', { method: 'POST' });
	window.location.href = '/login';
});

var mailTestBtn = document.getElementById('mailTestBtn');
if (mailTestBtn) {
	mailTestBtn.addEventListener('click', async function () {
		var msg = document.getElementById('mailTestMsg');
		mailTestBtn.disabled = true;
		msg.textContent = 'Encolando…';
		try {
			var response = await fetch('/admin/mail-test', { method: 'POST' });
			msg.textContent = await response.text();
		} catch (error) {
			msg.textContent = 'Error de conexión — intentá de nuevo.';
		} finally {
			mailTestBtn.disabled = false;
		}
	});
}
