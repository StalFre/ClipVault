// Detecta videos de YouTube (incluida la navegacion SPA) y ofrece enviarlos a ClipVault
// solo si la aplicacion esta abierta.
(() => {
  const ID = 'clipvault-toast';
  let lastKey = null;
  let hideTimer = null;

  function videoInfo() {
    const u = new URL(location.href);
    let id = null;
    if (u.pathname === '/watch') id = u.searchParams.get('v');
    else if (u.pathname.startsWith('/shorts/')) id = u.pathname.split('/')[2];
    if (!id) return null;
    const list = u.searchParams.get('list');
    return { id, list, url: location.href };
  }

  function remove() {
    clearTimeout(hideTimer);
    const el = document.getElementById(ID);
    if (el) el.remove();
  }

  function btn(label, primary, onClick) {
    const b = document.createElement('button');
    b.textContent = label;
    b.style.cssText =
      'border:0;border-radius:18px;padding:8px 16px;font:500 13px Roboto,Arial,sans-serif;cursor:pointer;' +
      (primary ? 'background:#9ecaff;color:#003258;' : 'background:#3b4858;color:#d7e3f8;');
    b.onclick = onClick;
    return b;
  }

  function show(info) {
    remove();
    const box = document.createElement('div');
    box.id = ID;
    box.style.cssText =
      'position:fixed;right:24px;bottom:24px;z-index:2147483647;background:#1c2024;color:#e0e2e8;' +
      'border-radius:20px;padding:16px 18px;box-shadow:0 8px 28px rgba(0,0,0,.45);max-width:320px;' +
      'font:14px Roboto,Arial,sans-serif;border:1px solid #43474e;';
    const title = document.createElement('div');
    title.textContent = 'ClipVault';
    title.style.cssText = 'font-weight:600;font-size:15px;color:#9ecaff;margin-bottom:4px;';
    const text = document.createElement('div');
    text.textContent = '¿Descargar este video?';
    text.style.cssText = 'margin-bottom:12px;color:#c3c7cf;';
    const row = document.createElement('div');
    row.style.cssText = 'display:flex;gap:8px;flex-wrap:wrap;';

    const done = (ok) => {
      text.textContent = ok ? 'Enviado a ClipVault ✓' : 'No se pudo conectar con ClipVault';
      row.remove();
      hideTimer = setTimeout(remove, 2000);
    };
    const go = (mode) =>
      chrome.runtime.sendMessage({ type: 'send', url: info.url, mode }, (r) => done(!!(r && r.ok)));

    row.append(btn('Descargar', true, () => go('single')));
    if (info.list) row.append(btn('Lista completa', false, () => go('playlist')));
    row.append(btn('Cerrar', false, remove));
    box.append(title, text, row);
    document.documentElement.appendChild(box);
    hideTimer = setTimeout(remove, 15000);
  }

  function check() {
    const info = videoInfo();
    if (!info) {
      lastKey = null;
      remove();
      return;
    }
    const key = info.id + '|' + (info.list || '');
    if (key === lastKey) return;
    lastKey = key;
    chrome.runtime.sendMessage({ type: 'ping' }, (r) => {
      if (chrome.runtime.lastError || !r || !r.ok) return; // app cerrada: sin aviso
      if (videoInfo() && lastKey === key) show(info);
    });
  }

  window.addEventListener('yt-navigate-finish', check);
  setInterval(check, 1500); // respaldo por si el evento no se dispara
  check();
})();
