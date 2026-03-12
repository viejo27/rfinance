document.getElementById('logoutBtn').addEventListener('click', async function() {
	await fetch('/api/logout', {
		method: 'POST'
	});
	window.location.href = '/login';
});
