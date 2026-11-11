// El service worker hace las peticiones a la app local (evita CORS/CSP de la pagina).
const BASE = 'http://127.0.0.1:5000';

async function ping() {
  try {
    const r = await fetch(BASE + '/api/ping', { cache: 'no-store' });
    return r.ok;
  } catch (e) {
    return false;
  }
}

async function send(url, mode) {
  try {
    const r = await fetch(BASE + '/api/extension/send', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ url, mode })
    });
    return r.ok;
  } catch (e) {
    return false;
  }
}

chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (msg.type === 'ping') ping().then(ok => sendResponse({ ok }));
  else if (msg.type === 'send') send(msg.url, msg.mode).then(ok => sendResponse({ ok }));
  else return false;
  return true; // respuesta asincrona
});
