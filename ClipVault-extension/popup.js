const sendBtn = document.getElementById('sendBtn');
const titleEl = document.getElementById('pageTitle');
const statusEl = document.getElementById('status');
let currentUrl = null;

function setStatus(msg, kind) {
  statusEl.textContent = msg;
  statusEl.className = kind || '';
}

chrome.tabs.query({ active: true, currentWindow: true }, function (tabs) {
  const tab = tabs[0];
  if (!tab || !tab.url) return;
  const isYoutube = tab.url.includes('youtube.com') || tab.url.includes('youtu.be');
  if (!isYoutube) {
    titleEl.textContent = 'Esta pagina no es de YouTube.';
    return;
  }
  currentUrl = tab.url;
  titleEl.textContent = tab.title || tab.url;
  sendBtn.disabled = false;
});

sendBtn.addEventListener('click', async function () {
  if (!currentUrl) return;
  sendBtn.disabled = true;
  sendBtn.textContent = 'Enviando...';
  try {
    const res = await fetch('http://127.0.0.1:5000/api/extension/send', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ url: currentUrl })
    });
    if (!res.ok) throw new Error('HTTP ' + res.status);
    setStatus('Enviado a ClipVault correctamente.', 'ok');
    sendBtn.textContent = 'Enviado ✓';
    setTimeout(() => window.close(), 900);
  } catch (err) {
    setStatus('No se pudo conectar. Abre ClipVault primero.', 'err');
    sendBtn.disabled = false;
    sendBtn.textContent = 'Reintentar';
  }
});
