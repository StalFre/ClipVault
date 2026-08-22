const socket = io();
function closeFfmpegModal() {
  document.getElementById('ffmpegModal').style.display = 'none';
}

function downloadFfmpeg() {
  fetch('/api/open-url', {method:'POST', headers:{'Content-Type':'application/json'},
    body: JSON.stringify({url: 'https://www.gyan.dev/ffmpeg/builds/'})});
}
let currentItems = [];
let currentMode = 'single';
let profiles = [];
let activeProfileId = null;
let editingProfileId = null;
let queueMap = {};
let dragItemId = null;
let currentSkipped = [];
let currentPlaylistTitle = '';

const BITRATE_KBPS = { 2160:35000, 1440:16000, 1080:8000, 720:5000, 480:2500, null:8000 };
const AUDIO_BITRATE_KBPS = { mp3:192, m4a:192, wav:1411, flac:800, opus:160 };

function installFfmpegWinget() {
  const btn = document.getElementById('installFfmpegBtn');
  const log = document.getElementById('ffmpegInstallLog');
  btn.disabled = true;
  btn.textContent = 'Instalando...';
  log.style.display = 'block';
  log.textContent = '';
  fetch('/api/install-ffmpeg', {method:'POST'});
}

socket.on('ffmpeg_install_log', function(d) {
  const log = document.getElementById('ffmpegInstallLog');
  log.textContent += d.line + '\n';
  log.scrollTop = log.scrollHeight;
});

socket.on('ffmpeg_install_done', function(d) {
  const btn = document.getElementById('installFfmpegBtn');
  btn.disabled = false;
  btn.textContent = 'Instalar con Winget';
  log('ffmpeg install: ' + d.message);
  if (d.ok && d.path) {
    showToast('ffmpeg instalado correctamente.', 'ok');
    closeFfmpegModal();
    checkFfmpeg();
  } else {
    showToast(d.message, 'error');
  }
});


function log(msg) {
  const el = document.getElementById('log');
  el.textContent = '[' + new Date().toLocaleTimeString() + '] ' + msg + '\n' + el.textContent;
}

function toggleTheme() {
  const html = document.documentElement;
  const next = html.getAttribute('data-theme') === 'dark' ? 'light' : 'dark';
  html.setAttribute('data-theme', next);
  document.getElementById('themeBtn').textContent = next === 'dark' ? '🌙 Modo oscuro' : '☀️ Modo claro';
  localStorage.setItem('theme', next);
  fetch('/api/settings', {method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({theme: next})});
}
(function initTheme(){
  const saved = localStorage.getItem('theme') || 'dark';
  document.documentElement.setAttribute('data-theme', saved);
  document.getElementById('themeBtn').textContent = saved === 'dark' ? '🌙 Modo oscuro' : '☀️ Modo claro';
})();

function switchTab(name) {
  document.querySelectorAll('nav.tabs button').forEach(function(b){ b.classList.toggle('active', b.dataset.tab === name); });
  document.querySelectorAll('.tabcontent').forEach(function(c){ c.classList.toggle('active', c.id === 'tab-' + name); });
  if (name === 'downloads') { loadQueue(); loadDiskSpace(); loadHistory(); }
}

function setMode(mode) {
  currentMode = mode;
  document.getElementById('modeSingleCard').classList.toggle('active', mode === 'single');
  document.getElementById('modePlaylistCard').classList.toggle('active', mode === 'playlist');
  document.getElementById('modeBulkCard').classList.toggle('active', mode === 'bulk');
  document.getElementById('singleInputRow').style.display = mode === 'bulk' ? 'none' : 'flex';
  document.getElementById('bulkInputRow').style.display = mode === 'bulk' ? 'block' : 'none';
}
function clearAll() {
  document.getElementById('url').value = '';
  document.getElementById('bulkUrls').value = '';
  currentItems = [];
  document.getElementById('listCard').style.display = 'none';
  document.getElementById('skippedSection').style.display = 'none';
  log('Lista y campos limpiados.');
}
function clearList() {
  currentItems = [];
  currentSkipped = [];
  document.getElementById('listCard').style.display = 'none';
  document.getElementById('skippedSection').style.display = 'none';
  log('Lista de resultados limpiada.');
}
function removeUnselected() {
  const checkedIds = Array.from(document.querySelectorAll('.item input[type=checkbox]:checked')).map(function(cb){ return cb.dataset.id; });
  const removed = currentItems.length - checkedIds.length;
  if (removed <= 0) { log('No hay elementos deseleccionados para quitar.'); return; }
  currentItems = currentItems.filter(function(i){ return checkedIds.indexOf(i.id) !== -1; });
  renderList();
  estimateAndShowPreview();
  log('Se quitaron ' + removed + ' elemento(s) no seleccionado(s) de la lista.');
}
function clearSkipped() {
  currentSkipped = [];
  document.getElementById('skippedSection').style.display = 'none';
  document.getElementById('skippedListContainer').innerHTML = '';
  log('Lista de videos con problemas vaciada.');
}
function renderSkippedList(skipped, totalCount) {
  currentSkipped = skipped || [];
  const section = document.getElementById('skippedSection');
  const summary = document.getElementById('skippedSummary');
  const container = document.getElementById('skippedListContainer');
  if (!currentSkipped.length) { section.style.display = 'none'; return; }
  section.style.display = 'block';
  summary.textContent = 'De ' + totalCount + ' elementos, ' + currentSkipped.length + ' tienen problemas (privados, eliminados o no disponibles) y fueron excluidos de la descarga.';
  container.innerHTML = '';
  currentSkipped.forEach(function(s) {
    const div = document.createElement('div');
    div.className = 'skip-item';
    div.innerHTML = '<div class="skip-icon">⚠</div>' +
      '<div class="skip-info">' +
        '<div class="skip-title">' + s.title + '</div>' +
        '<div class="skip-reason">' + s.reason + '</div>' +
        (s.url ? '<div class="skip-url">' + s.url + '</div>' : '') +
      '</div>';
    container.appendChild(div);
  });
}

let ffmpegModalShown = false;

function checkFfmpeg() {
  fetch('/api/check-ffmpeg').then(function(res){ return res.json(); }).then(function(data){
    const b = document.getElementById('banner');
    if (!data.likely_ok) {
      b.style.display = 'block';
      b.textContent = 'ffmpeg no detectado. Configuralo en Configuracion.';
      if (!ffmpegModalShown) {
        ffmpegModalShown = true;
        document.getElementById('ffmpegModal').style.display = 'flex';
      }
    } else {
      b.style.display = 'none';
    }
  });
}

function formatDuration(totalSeconds) {
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  const s = Math.floor(totalSeconds % 60);
  if (h > 0) return h + 'h ' + m + 'm';
  if (m > 0) return m + 'm ' + s + 's';
  return s + 's';
}

function currentProfileObj() {
  return profiles.find(function(p){ return p.id === activeProfileId; }) || null;
}

function estimateAndShowPreview() {
  const totalSeconds = currentItems.reduce(function(sum, it){ return sum + (it.duration || 0); }, 0);
  const profile = currentProfileObj();
  let kbps = 8000;
  if (profile) {
    if (profile.kind === 'audio') kbps = AUDIO_BITRATE_KBPS[profile.audio_codec] || 192;
    else kbps = BITRATE_KBPS[profile.height] || BITRATE_KBPS[null];
  }
  const estMB = (totalSeconds * kbps) / 8 / 1024;
  const estGB = estMB / 1024;
  const sizeText = estGB >= 1 ? estGB.toFixed(2) + ' GB' : Math.round(estMB) + ' MB';
  const el = document.getElementById('previewInfo');
  el.textContent = 'Duracion total: ' + formatDuration(totalSeconds) + '  -  Tamano estimado (' + (profile ? profile.name : '?') + '): ~' + sizeText + ' (aproximado)';
}

function extract() {
  const url = document.getElementById('url').value.trim();
  if (!url) return;
  log('Extrayendo informacion...');
  fetch('/api/extract', {method:'POST', headers:{'Content-Type':'application/json'},
    body: JSON.stringify({url: url, single: currentMode === 'single'})})
    .then(function(res){ return res.json(); })
    .then(function(data){
      if (data.error) { log('Error: ' + data.error); return; }
      currentItems = data.items;
      document.getElementById('listCard').style.display = 'block';
      document.getElementById('playlistTitle').textContent = data.playlist_title
        ? ('Playlist: ' + data.playlist_title + ' (' + data.items.length + ')') : 'Video individual';
              currentPlaylistTitle = data.playlist_title || '';
      renderList();
      renderSkippedList(data.skipped || [], data.total_count || 0);
      estimateAndShowPreview();
      log('Se encontraron ' + data.items.length + ' elemento(s).');
    });
}

function extractBulk() {
  const raw = document.getElementById('bulkUrls').value;
  const urls = raw.split('\n').map(function(u){ return u.trim(); }).filter(Boolean);
  if (!urls.length) return;
  log('Extrayendo ' + urls.length + ' enlace(s)...');
  fetch('/api/extract-bulk', {method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({urls: urls})})
    .then(function(res){ return res.json(); })
    .then(function(data){
      currentItems = data.items;
      document.getElementById('listCard').style.display = 'block';
      document.getElementById('playlistTitle').textContent = 'Enlaces multiples (' + data.items.length + ' de ' + urls.length + ')';
      renderList();
      renderSkippedList(data.skipped || [], data.total_count || 0);
      estimateAndShowPreview();
      (data.failed || []).forEach(function(f){ log('Fallo extraccion: ' + f.url + ' - ' + f.error); });
    });
}

let currentSearchItems = [];

function searchYoutube() {
  const query = document.getElementById('searchQuery').value.trim();
  if (!query) return;
  const maxResults = document.getElementById('searchMaxResults').value;
  log('Buscando "' + query + '"...');
  fetch('/api/search', {method:'POST', headers:{'Content-Type':'application/json'},
    body: JSON.stringify({query: query, max_results: maxResults})})
    .then(function(res){ return res.json(); })
    .then(function(data){
      if (data.error) { log('Error: ' + data.error); return; }
      currentSearchItems = data.items;
      document.getElementById('searchResultsCard').style.display = 'block';
      document.getElementById('searchResultsTitle').textContent = 'Resultados para "' + query + '" (' + data.items.length + ')';
      renderSearchList();
      log('Se encontraron ' + data.items.length + ' resultado(s).');
    });
}

function renderSearchList() {
  const list = document.getElementById('searchList');
  list.innerHTML = '';
  currentSearchItems.forEach(function(item) {
    const div = document.createElement('div');
    div.className = 'item'; div.id = 'search-item-' + item.id;
    const dur = item.duration ? new Date(item.duration*1000).toISOString().substr(14,5) : '--:--';
    div.innerHTML = '<input type="checkbox" checked data-id="' + item.id + '">' +
      '<div class="thumbwrap"><span class="placeholder">VIDEO</span><img data-fallback-tried="0" data-vid="' + (item.id||'') + '" src="' + (item.thumbnail || '') + '"></div>' +
      '<div style="flex:1;"><div class="title">' + item.title + '</div></div>' +
      '<div class="dur">' + dur + '</div>';
    const img = div.querySelector('img');
    img.addEventListener('error', function(){ handleThumbError(img); });
    img.addEventListener('mouseenter', function(ev){ showPreview(item, ev); });
    img.addEventListener('mousemove', function(ev){ positionPreview(ev); });
    img.addEventListener('mouseleave', function(){ hidePreview(); });
    list.appendChild(div);
  });
}

function selectAllSearch(state) {
  document.querySelectorAll('#searchList input[type=checkbox]').forEach(function(cb){ cb.checked = state; });
}

function startSearchDownload() {
  const ids = Array.from(document.querySelectorAll('#searchList input[type=checkbox]:checked')).map(function(cb){ return cb.dataset.id; });
  const items = currentSearchItems.filter(function(i){ return ids.indexOf(i.id) !== -1; });
  if (!items.length) { log('No hay resultados seleccionados.'); return; }
  fetch('/api/download', {method:'POST', headers:{'Content-Type':'application/json'},
    body: JSON.stringify({items: items, profile_id: activeProfileId, playlist_title: ''})})
    .then(function(res){ return res.json(); })
    .then(function(data){
      if (data.error) { log('Error: ' + data.error); return; }
      log(data.total + ' elemento(s) agregados a la cola desde busqueda.');
      switchTab('downloads');
    });
}

function clearSearch() {
  document.getElementById('searchQuery').value = '';
  currentSearchItems = [];
  document.getElementById('searchResultsCard').style.display = 'none';
  document.getElementById('searchList').innerHTML = '';
  log('Busqueda y resultados limpiados.');
}

document.getElementById('searchQuery').addEventListener('keydown', function(e) {
  if (e.key === 'Enter') { e.preventDefault(); searchYoutube(); }
});

function renderList() {
  const list = document.getElementById('list');
  list.innerHTML = '';
  currentItems.forEach(function(item) {
    const div = document.createElement('div');
    div.className = 'item'; div.id = 'item-' + item.id;
    const dur = item.duration ? new Date(item.duration*1000).toISOString().substr(14,5) : '--:--';
    div.innerHTML = '<input type="checkbox" checked data-id="' + item.id + '">' +
      '<div class="thumbwrap"><span class="placeholder">VIDEO</span><img data-fallback-tried="0" data-vid="' + (item.id||'') + '" src="' + (item.thumbnail || '') + '"></div>' +
      '<div style="flex:1;"><div class="title">' + item.title + '</div></div>' +
      '<div class="dur">' + dur + '</div>';
    const img = div.querySelector('img');
    img.addEventListener('error', function(){ handleThumbError(img); });
    img.addEventListener('mouseenter', function(ev){ showPreview(item, ev); });
    img.addEventListener('mousemove', function(ev){ positionPreview(ev); });
    img.addEventListener('mouseleave', function(){ hidePreview(); });
    list.appendChild(div);
  });
}
function handleThumbError(img) {
  const tried = img.dataset.fallbackTried;
  const vid = img.dataset.vid;
  if (tried === '0' && vid) {
    img.dataset.fallbackTried = '1';
    img.src = 'https://i.ytimg.com/vi/' + vid + '/hqdefault.jpg';
  } else if (tried === '1' && vid) {
    img.dataset.fallbackTried = '2';
    img.src = 'https://i.ytimg.com/vi/' + vid + '/mqdefault.jpg';
  } else {
    img.style.display = 'none';
  }
}
function selectAll(state) { document.querySelectorAll('.item input[type=checkbox]').forEach(function(cb){ cb.checked = state; }); }

function showPreview(item, ev) {
  const pop = document.getElementById('previewPop');
  const inner = document.getElementById('previewPopInner');
  const isYoutube = (item.extractor || '').indexOf('youtube') !== -1 || (item.url || '').indexOf('youtube') !== -1 || (item.url || '').indexOf('youtu.be') !== -1;
  if (isYoutube && item.id) {
    inner.innerHTML = '<iframe src="https://www.youtube.com/embed/' + item.id +
      '?autoplay=1&mute=1&controls=0&loop=1&playlist=' + item.id +
      '" allow="autoplay; encrypted-media" frameborder="0"></iframe>';
  } else if (item.thumbnail) {
    inner.innerHTML = '<img src="' + item.thumbnail + '" style="width:100%;height:100%;object-fit:cover;">';
  } else {
    inner.innerHTML = '<div style="color:#888;font-size:0.8rem;padding:10px;">Sin vista previa disponible para este sitio</div>';
  }
  pop.style.display = 'block';
  positionPreview(ev);
}
function positionPreview(ev) {
  const pop = document.getElementById('previewPop');
  let x = ev.clientX + 20;
  let y = ev.clientY - 90;
  if (x + 320 > window.innerWidth) x = ev.clientX - 340;
  if (y < 10) y = 10;
  pop.style.left = x + 'px';
  pop.style.top = y + 'px';
}
function hidePreview() {
  const pop = document.getElementById('previewPop');
  pop.style.display = 'none';
  document.getElementById('previewPopInner').innerHTML = '';
}

function startDownload() {
  const ids = Array.from(document.querySelectorAll('.item input[type=checkbox]:checked')).map(function(cb){ return cb.dataset.id; });
  const items = currentItems.filter(function(i){ return ids.indexOf(i.id) !== -1; });
  if (!items.length) { log('No hay videos seleccionados.'); return; }
  fetch('/api/download', {method:'POST', headers:{'Content-Type':'application/json'},
    body: JSON.stringify({items: items, profile_id: activeProfileId, playlist_title: currentPlaylistTitle})})
    .then(function(res){ return res.json(); })
    .then(function(data){
      if (data.error) { log('Error: ' + data.error); return; }
      log(data.total + ' elemento(s) agregados a la cola. Espacio libre: ' + (data.free_disk_gb != null ? data.free_disk_gb : '?') + ' GB.');
      switchTab('downloads');
    });
}

function loadQueue() {
  fetch('/api/queue').then(function(res){ return res.json(); }).then(function(data){
    queueMap = {};
    data.items.forEach(function(it){ queueMap[it.item_id] = it; });
    renderQueue();
  });
}
function friendlyError(raw) {
  if (!raw) return '';
  const msg = raw.toLowerCase();
  if (msg.indexOf('claimed content') !== -1 || msg.indexOf('copyright') !== -1) return 'Reclamo de derechos de autor: no se puede descargar.';
  if (msg.indexOf('private video') !== -1) return 'Este video es privado.';
  if (msg.indexOf('members-only') !== -1) return 'Contenido exclusivo para miembros.';
  if (msg.indexOf('not available in your country') !== -1) return 'No disponible en tu region.';
  if (msg.indexOf('has been removed') !== -1) return 'Este video fue eliminado.';
  if (msg.indexOf('video unavailable') !== -1 || msg.indexOf('this video is unavailable') !== -1) return 'Este video ya no esta disponible.';
  if (msg.indexOf('429') !== -1 || msg.indexOf('too many requests') !== -1) return 'Demasiadas solicitudes a YouTube. Intenta mas tarde.';
  if (msg.indexOf('espacio en disco') !== -1) return raw;
  return 'No se pudo descargar este video.';
}
function renderQueue() {
  const container = document.getElementById('queueList');
  const rows = Object.values(queueMap).sort(function(a,b){ return a.order - b.order; });
  const clearRow = document.getElementById('clearQueueRow');
  if (!rows.length) {
    container.innerHTML = '<div style="color:var(--sub); font-size:0.85rem;">La cola esta vacia.</div>';
    clearRow.style.display = 'none';
    return;
  }
  clearRow.style.display = 'flex';
  container.innerHTML = '';
  rows.forEach(function(it) {
    const div = document.createElement('div');
    div.className = 'qrow'; div.id = 'qrow-' + it.item_id; div.dataset.itemId = it.item_id;
    const isCancelling = it.status === 'cancelling';
    const pauseBtn = it.status === 'paused'
      ? '<button class="btn icon-sm" onclick="resumeItem(\'' + it.item_id + '\')"' + (isCancelling ? ' disabled' : '') + '>▶ Reanudar</button>'
      : '<button class="btn icon-sm" onclick="pauseItem(\'' + it.item_id + '\')"' + (isCancelling ? ' disabled' : '') + '>⏸ Pausar</button>';
    const thumbHtml = it.thumbnail
      ? '<img data-fallback-tried="0" data-vid="' + (it.video_id||'') + '" src="' + it.thumbnail + '">'
      : '<span class="placeholder">VIDEO</span>';
    div.innerHTML =
      '<div class="drag-handle" draggable="true" title="Arrastra para reordenar"><span></span><span></span><span></span><span></span><span></span><span></span></div>' +
      '<div class="qthumb">' + thumbHtml + '</div>' +
      '<div class="qtitle">' + it.title + '</div>' +
      '<div class="bar"><div class="bar-fill" style="width:' + (it.pct||0) + '%"></div></div>' +
      '<div class="qstatus ' + it.status + '" title="' + (it.error ? it.error.replace(/"/g,'&quot;') : '') + '">' + it.status + (it.error ? (' - ' + friendlyError(it.error)) : '') + '</div>' +
      '<div class="row" style="gap:4px;">' + pauseBtn +
        '<button class="btn icon-sm" onclick="cancelItem(\'' + it.item_id + '\')"' + (isCancelling ? ' disabled' : '') + '>✕ ' + (isCancelling ? 'Cancelando...' : 'Cancelar') + '</button>' +
      '</div>';
    const img = div.querySelector('.qthumb img');
    if (img) { img.addEventListener('error', function(){ handleQueueThumbError(img); }); }
    const handle = div.querySelector('.drag-handle');
    handle.addEventListener('dragstart', function(){ dragItemId = it.item_id; div.classList.add('dragging'); });
    handle.addEventListener('dragend', function(){ div.classList.remove('dragging'); });
    div.addEventListener('dragover', function(e){ e.preventDefault(); div.classList.add('drag-over'); });
    div.addEventListener('dragleave', function(){ div.classList.remove('drag-over'); });
    div.addEventListener('drop', function(e){
      e.preventDefault(); div.classList.remove('drag-over');
      if (!dragItemId || dragItemId === it.item_id) return;
      reorderByDrag(dragItemId, it.item_id);
      dragItemId = null;
    });
    container.appendChild(div);
  });
}
function handleQueueThumbError(img) {
  const tried = img.dataset.fallbackTried;
  const vid = img.dataset.vid;
  if (tried === '0' && vid) { img.dataset.fallbackTried = '1'; img.src = 'https://i.ytimg.com/vi/' + vid + '/hqdefault.jpg'; }
  else if (tried === '1' && vid) { img.dataset.fallbackTried = '2'; img.src = 'https://i.ytimg.com/vi/' + vid + '/mqdefault.jpg'; }
  else { img.style.display = 'none'; }
}
function reorderByDrag(draggedId, targetId) {
  const rows = Object.values(queueMap).sort(function(a,b){ return a.order - b.order; });
  const ids = rows.map(function(r){ return r.item_id; });
  const from = ids.indexOf(draggedId);
  const to = ids.indexOf(targetId);
  if (from === -1 || to === -1) return;
  ids.splice(from, 1);
  ids.splice(to, 0, draggedId);
  ids.forEach(function(id, idx){ if (queueMap[id]) queueMap[id].order = idx; });
  renderQueue();
  fetch('/api/queue/reorder-full', {method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({order: ids})});
}
function pauseItem(id) {
  const btn = document.querySelector('#qrow-' + id + ' button[onclick*="pauseItem"]');
  if (btn) { btn.disabled = true; }
  fetch('/api/queue/' + id + '/pause', {method:'POST'});
}
function resumeItem(id) {
  const btn = document.querySelector('#qrow-' + id + ' button[onclick*="resumeItem"]');
  if (btn) { btn.disabled = true; }
  fetch('/api/queue/' + id + '/resume', {method:'POST'});
}
function cancelItem(id) {
  const row = document.getElementById('qrow-' + id);
  if (row) row.querySelectorAll('button').forEach(function(b){ b.disabled = true; });
  fetch('/api/queue/' + id + '/cancel', {method:'POST'});
}
function clearQueue() {
  if (!confirm('Esto quitara de la cola los elementos terminados, cancelados y fallidos. Continuar?')) return;
  fetch('/api/queue/clear', {method:'POST'}).then(function(res){ return res.json(); }).then(function(data){
    log('Cola limpiada (' + (data.cleared || 0) + ' elementos quitados).');
    loadQueue();
  });
}

function loadDiskSpace() {
  fetch('/api/disk-space').then(function(res){ return res.json(); }).then(function(data){
    if (data.free_gb !== undefined) document.getElementById('diskBanner').textContent = '💾 Espacio libre en destino: ' + data.free_gb + ' GB de ' + data.total_gb + ' GB';
  });
}

function loadHistory() {
  const q = document.getElementById('histSearch').value.trim();
  const status = document.getElementById('histStatusFilter').value;
  const params = new URLSearchParams({ q: q, status: status });
  fetch('/api/history?' + params.toString()).then(function(res){ return res.json(); }).then(function(data){
    renderHistory(data.records);
  });
}
function renderHistory(records) {
  const wrap = document.getElementById('histTableWrap');
  const clearBtn = document.getElementById('clearHistBtn');
  if (!records.length) {
    wrap.innerHTML = '<div style="color:var(--sub); font-size:0.85rem;">Sin registros todavia.</div>';
    clearBtn.style.display = 'none';
    return;
  }
  clearBtn.style.display = 'inline-block';
  let html = '<table class="hist"><tr><th>Fecha</th><th>Miniatura</th><th>Titulo</th><th>Perfil</th><th>Estado</th><th>Acciones</th></tr>';
  records.forEach(function(r) {
    const statusClass = (r.status || '').split(' ')[0];
    const retryBtn = (r.status === 'failed' || r.status === 'failed (no recuperable)' || r.status === 'cancelled')
      ? '<button class="btn icon-sm" onclick="retryHistory(\'' + r.item_id + '\')">↻ Reintentar</button>' : '';
    const vid = r.video ? (r.video.id || '') : '';
    const thumb = r.video ? (r.video.thumbnail || '') : '';
    const thumbHtml = thumb
      ? '<div class="hthumb-wrap"><img data-fallback-tried="0" data-vid="' + vid + '" src="' + thumb + '"></div>'
      : '<div class="hthumb-wrap"><span class="placeholder">VIDEO</span></div>';
    html += '<tr>' +
      '<td>' + (r.finished_at || '').replace('T',' ') + '</td>' +
      '<td class="hthumb">' + thumbHtml + '</td>' +
      '<td style="max-width:260px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;">' + r.title + '</td>' +
      '<td>' + (r.profile_name || '') + '</td>' +
      '<td><span class="st ' + statusClass + '">' + r.status + '</span></td>' +
      '<td>' + retryBtn + ' <button class="btn icon-sm" onclick="openFolder(\'' + (r.dest_dir||'').replace(/\\/g,'\\\\') + '\')">📁 Abrir</button></td>' +
      '</tr>';
  });
  html += '</table>';
  wrap.innerHTML = html;
  wrap.querySelectorAll('.hthumb-wrap img').forEach(function(img){
    img.addEventListener('error', function(){ handleHistThumbError(img); });
  });
}
function handleHistThumbError(img) {
  const tried = img.dataset.fallbackTried;
  const vid = img.dataset.vid;
  if (tried === '0' && vid) { img.dataset.fallbackTried = '1'; img.src = 'https://i.ytimg.com/vi/' + vid + '/hqdefault.jpg'; }
  else if (tried === '1' && vid) { img.dataset.fallbackTried = '2'; img.src = 'https://i.ytimg.com/vi/' + vid + '/mqdefault.jpg'; }
  else { img.style.display = 'none'; }
}
function retryHistory(itemId) {
  fetch('/api/history/' + itemId + '/retry', {method:'POST'}).then(function(res){ return res.json(); }).then(function(data){
    if (data.error) { log('Error al reintentar: ' + data.error); return; }
    log('Reintento encolado.');
    loadQueue();
  });
}
function openFolder(path) {
  fetch('/api/open-folder', {method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({path: path})})
    .then(function(res){ return res.json(); }).then(function(data){ if (data.error) log('Error: ' + data.error); });
}
function clearHistory() {
  if (!confirm('Esto borrara todo el historial de descargas. Continuar?')) return;
  fetch('/api/history', {method:'DELETE'}).then(function(){ loadHistory(); });
}

socket.on('item_status', function(d) {
  const existed = queueMap[d.item_id];
  queueMap[d.item_id] = Object.assign({}, queueMap[d.item_id], d);
  const row = document.getElementById('qrow-' + d.item_id);
  if (row && existed && existed.status === d.status) {
    const fill = row.querySelector('.bar-fill');
    if (fill) fill.style.width = (d.pct || 0) + '%';
    const statusEl = row.querySelector('.qstatus');
    if (statusEl) {
      statusEl.textContent = d.status + (d.error ? (' - ' + friendlyError(d.error)) : '');
      statusEl.title = d.error ? d.error.replace(/"/g,'&quot;') : d.status;
    }
    return;
  }
  renderQueue();
});
socket.on('item_order', function(d) { if (queueMap[d.item_id]) { queueMap[d.item_id].order = d.order; } });
socket.on('queue_cleared', function() { loadQueue(); });
socket.on('item_removed', function(d) {
  delete queueMap[d.item_id];
  renderQueue();
});
socket.on('job_tick', function() { loadDiskSpace(); });
socket.on('history_updated', function() { if (document.getElementById('tab-downloads').classList.contains('active')) loadHistory(); });
socket.on('job_finished', function(d) { log('Job ' + d.job_id + ' completado (' + d.total + ' elemento(s)).'); loadHistory(); });

let pendingExtensionUrl = null;

socket.on('extension_link', function(d) {
  pendingExtensionUrl = d.url;
  document.getElementById('extensionLinkText').textContent = d.url;
  document.getElementById('extensionLinkModal').style.display = 'flex';
});

function closeExtensionLinkModal() {
  document.getElementById('extensionLinkModal').style.display = 'none';
  pendingExtensionUrl = null;
}

function handleExtensionLinkChoice(mode) {
  if (!pendingExtensionUrl) return;
  const url = pendingExtensionUrl;
  closeExtensionLinkModal();
  switchTab('download');
  setMode(mode);
  document.getElementById('url').value = url;
  showToast('Extrayendo enlace recibido...', 'ok');
  extract();
}


function browseFolder() {
  fetch('/api/browse-folder', {method:'POST'}).then(function(res){ return res.json(); }).then(function(data){
    if (data.path) {
      document.getElementById('destFolder').value = data.path;
      const df2 = document.getElementById('destFolderSearch');
      if (df2) df2.value = data.path;
      fetch('/api/settings', {method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({download_dir: data.path})});
    } else if (data.error) { log('Error selector: ' + data.error); }
  });
}

function loadProfiles() {
  fetch('/api/profiles').then(function(res){ return res.json(); }).then(function(data){
    profiles = data.profiles; activeProfileId = data.active_profile_id;
    renderProfiles();
    renderProfileSelect();
  });
}
function renderProfileSelect() {
  ['activeProfileSelect', 'activeProfileSelectSearch'].forEach(function(selId) {
    const sel = document.getElementById(selId);
    if (!sel) return;
    sel.innerHTML = '';
    const videoGroup = document.createElement('optgroup'); videoGroup.label = 'Video';
    const audioGroup = document.createElement('optgroup'); audioGroup.label = 'Audio';
    profiles.forEach(function(p) {
      const opt = document.createElement('option');
      opt.value = p.id; opt.textContent = p.name;
      if (p.id === activeProfileId) opt.selected = true;
      (p.kind === 'audio' ? audioGroup : videoGroup).appendChild(opt);
    });
    sel.appendChild(videoGroup); sel.appendChild(audioGroup);
  });
}
function onProfileSelectChange(selectId) {
  const id = document.getElementById(selectId).value;
  setActiveProfile(id);
  estimateAndShowPreview();
}
function profileCardHtml(p) {
  const actionBtn = p.builtin
    ? '<button class="btn" onclick="event.stopPropagation(); resetProfile(\'' + p.id + '\')">Reset</button>'
    : '<button class="btn" onclick="event.stopPropagation(); deleteProfile(\'' + p.id + '\')">Eliminar</button>';
  return '<div class="check">✓</div>' +
    '<div class="name">' + p.name + ' <span class="badge">' + (p.builtin ? 'Built-in' : 'Custom') + '</span></div>' +
    '<div class="pdesc">' + (p.desc || '') + '</div>' +
    '<div class="profile-actions">' +
      '<button class="btn" onclick="event.stopPropagation(); editProfile(\'' + p.id + '\')">Editar</button>' + actionBtn +
    '</div>';
}
function renderProfiles() {
  const gridV = document.getElementById('profilesGridVideo');
  const gridA = document.getElementById('profilesGridAudio');
  gridV.innerHTML = ''; gridA.innerHTML = '';
  profiles.forEach(function(p) {
    const div = document.createElement('div');
    div.className = 'profile-card' + (p.id === activeProfileId ? ' active' : '');
    div.innerHTML = profileCardHtml(p);
    div.onclick = function(){ setActiveProfile(p.id); renderProfileSelect(); };
    (p.kind === 'audio' ? gridA : gridV).appendChild(div);
  });
}
function setActiveProfile(id) {
  activeProfileId = id; renderProfiles(); renderProfileSelect();
  fetch('/api/profiles/active', {method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({profile_id:id})});
}
function openProfileModal() {
  editingProfileId = null;
  document.getElementById('modalTitle').textContent = 'Nuevo perfil';
  document.getElementById('pfName').value = ''; document.getElementById('pfHeight').value = '';
  document.getElementById('pfContainer').value = 'native'; document.getElementById('pfAudioCodec').value = 'mp3';
  setKind('video');
  document.getElementById('profileModal').style.display = 'flex';
}
function editProfile(id) {
  const p = profiles.find(function(x){ return x.id === id; }); editingProfileId = id;
  document.getElementById('modalTitle').textContent = 'Editar perfil';
  document.getElementById('pfName').value = p.name; document.getElementById('pfHeight').value = p.height || '';
  document.getElementById('pfContainer').value = p.container || 'native';
  document.getElementById('pfAudioCodec').value = p.audio_codec || 'mp3';
  setKind(p.kind);
  document.getElementById('profileModal').style.display = 'flex';
}
function closeProfileModal() { document.getElementById('profileModal').style.display = 'none'; }
function setKind(kind) {
  document.querySelectorAll('#pfKindToggle button').forEach(function(b){ b.classList.toggle('active', b.dataset.val === kind); });
  document.getElementById('pfHeightField').style.display = kind === 'audio' ? 'none' : 'block';
  document.getElementById('pfContainerField').style.display = kind === 'audio' ? 'none' : 'block';
  document.getElementById('pfAudioCodecField').style.display = kind === 'audio' ? 'block' : 'none';
}
document.getElementById('pfKindToggle').addEventListener('click', function(e){ if (e.target.dataset.val) setKind(e.target.dataset.val); });
function saveProfile() {
  const kind = document.querySelector('#pfKindToggle button.active').dataset.val;
  const audioLabels = {mp3:'MP3', m4a:'M4A/AAC', wav:'WAV', flac:'FLAC', opus:'Opus original'};
  const codec = document.getElementById('pfAudioCodec').value;
  const heightVal = document.getElementById('pfHeight').value;
  const body = { id: editingProfileId, name: document.getElementById('pfName').value || 'Perfil personalizado',
    kind: kind, height: heightVal ? parseInt(heightVal) : null,
    container: document.getElementById('pfContainer').value,
    audio_codec: codec,
    desc: kind === 'audio' ? ('Audio only - ' + audioLabels[codec]) : ('Up to ' + (heightVal || 'best') + 'p') };
  fetch('/api/profiles', {method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify(body)})
    .then(function(){ closeProfileModal(); loadProfiles(); });
}
function resetProfile(id) { fetch('/api/profiles/' + id + '/reset', {method:'POST'}).then(function(){ loadProfiles(); }); }
function deleteProfile(id) { fetch('/api/profiles/' + id, {method:'DELETE'}).then(function(){ loadProfiles(); }); }

document.getElementById('cookiesModeToggle').addEventListener('click', function(e) {
  if (!e.target.dataset.val) return;
  document.querySelectorAll('#cookiesModeToggle button').forEach(function(b){ b.classList.toggle('active', b === e.target); });
  document.getElementById('cookiesFileField').style.display = e.target.dataset.val === 'file' ? 'block' : 'none';
  document.getElementById('cookiesBrowserField').style.display = e.target.dataset.val === 'browser' ? 'block' : 'none';
});
document.getElementById('cautiousToggle').addEventListener('click', function(e) {
  if (!e.target.dataset.val) return;
  document.querySelectorAll('#cautiousToggle button').forEach(function(b){ b.classList.toggle('active', b === e.target); });
});
document.getElementById('rateLimitToggle').addEventListener('click', function(e) {
  if (!e.target.dataset.val) return;
  document.querySelectorAll('#rateLimitToggle button').forEach(function(b){ b.classList.toggle('active', b === e.target); });
});
document.getElementById('concurrencyToggle').addEventListener('click', function(e) {
  if (!e.target.dataset.val) return;
  document.querySelectorAll('#concurrencyToggle button').forEach(function(b){ b.classList.toggle('active', b === e.target); });
});
document.getElementById('notifToggle').addEventListener('click', function(e) {
  if (!e.target.dataset.val) return;
  document.querySelectorAll('#notifToggle button').forEach(function(b){ b.classList.toggle('active', b === e.target); });
});
function toggleSponsor() {
  const btn = document.getElementById('sponsorBtn');
  const isOn = btn.dataset.val === 'on';
  const newVal = isOn ? 'off' : 'on';
  btn.dataset.val = newVal;
  btn.textContent = newVal === 'on' ? '🚫 Sin patrocinios (on)' : '🚫 Sin patrocinios (off)';
  btn.classList.toggle('primary', newVal === 'on');
}
document.getElementById('embedToggle').addEventListener('click', function(e) {
  if (!e.target.dataset.val) return;
  document.querySelectorAll('#embedToggle button').forEach(function(b){ b.classList.toggle('active', b === e.target); });
});
document.getElementById('normalizeToggle').addEventListener('click', function(e) {
  if (!e.target.dataset.val) return;
  document.querySelectorAll('#normalizeToggle button').forEach(function(b){ b.classList.toggle('active', b === e.target); });
});
document.getElementById('subsToggle').addEventListener('click', function(e) {
  if (!e.target.dataset.val) return;
  document.querySelectorAll('#subsToggle button').forEach(function(b){ b.classList.toggle('active', b === e.target); });
  document.getElementById('subsLangsField').style.display = e.target.dataset.val === 'on' ? 'block' : 'none';
});

function loadSettings() {
  fetch('/api/settings').then(function(res){ return res.json(); }).then(function(s){
    document.getElementById('destFolder').value = s.download_dir || '';
    const df2 = document.getElementById('destFolderSearch');
    if (df2) df2.value = s.download_dir || '';
    document.getElementById('proxyUrl').value = s.proxy_url || '';
    document.getElementById('playlistLimit').value = s.playlist_scan_limit || 100;
    document.getElementById('ffmpegLocation').value = s.ffmpeg_location || '';
    document.getElementById('cookiesFile').value = s.cookies_file || '';
    document.getElementById('cookiesBrowser').value = s.cookies_browser || 'chrome';
    document.getElementById('subtitleLangs').value = s.subtitle_langs || 'es,en';
    document.querySelectorAll('#cookiesModeToggle button').forEach(function(b){ b.classList.toggle('active', b.dataset.val === s.cookies_mode); });
    document.getElementById('cookiesFileField').style.display = s.cookies_mode === 'file' ? 'block' : 'none';
    document.getElementById('cookiesBrowserField').style.display = s.cookies_mode === 'browser' ? 'block' : 'none';
    document.querySelectorAll('#cautiousToggle button').forEach(function(b){ b.classList.toggle('active', b.dataset.val === s.cautious_mode); });
    document.querySelectorAll('#rateLimitToggle button').forEach(function(b){ b.classList.toggle('active', b.dataset.val === String(s.rate_limit_kbps || 0)); });
    document.querySelectorAll('#concurrencyToggle button').forEach(function(b){ b.classList.toggle('active', b.dataset.val === String(s.concurrency)); });
    const notifVal = s.notifications_enabled === false ? 'off' : 'on';
    document.querySelectorAll('#notifToggle button').forEach(function(b){ b.classList.toggle('active', b.dataset.val === notifVal); });
    const sponsorVal = s.sponsorblock_remove ? 'on' : 'off';
    const sponsorBtn = document.getElementById('sponsorBtn');
    sponsorBtn.dataset.val = sponsorVal;
    sponsorBtn.textContent = sponsorVal === 'on' ? '🚫 Sin patrocinios (on)' : '🚫 Sin patrocinios (off)';
    sponsorBtn.classList.toggle('primary', sponsorVal === 'on');
    const embedVal = s.embed_thumbnail_metadata ? 'on' : 'off';
    document.querySelectorAll('#embedToggle button').forEach(function(b){ b.classList.toggle('active', b.dataset.val === embedVal); });
    const normVal = s.normalize_volume ? 'on' : 'off';
    document.querySelectorAll('#normalizeToggle button').forEach(function(b){ b.classList.toggle('active', b.dataset.val === normVal); });
    const subsVal = s.download_subtitles ? 'on' : 'off';
    document.querySelectorAll('#subsToggle button').forEach(function(b){ b.classList.toggle('active', b.dataset.val === subsVal); });
    document.getElementById('subsLangsField').style.display = subsVal === 'on' ? 'block' : 'none';
  });
}
function showToast(message, kind) {
  const container = document.getElementById('toastContainer');
  const el = document.createElement('div');
  el.className = 'toast-item ' + (kind === 'error' ? 'err' : 'ok');
  el.textContent = message;
  container.appendChild(el);
  setTimeout(function(){ el.remove(); }, 3000);
}

function saveSettings() {
  const body = {
    proxy_url: document.getElementById('proxyUrl').value,
    playlist_scan_limit: parseInt(document.getElementById('playlistLimit').value) || 100,
    ffmpeg_location: document.getElementById('ffmpegLocation').value,
    cookies_mode: document.querySelector('#cookiesModeToggle button.active').dataset.val,
    cookies_file: document.getElementById('cookiesFile').value,
    cookies_browser: document.getElementById('cookiesBrowser').value,
    cautious_mode: document.querySelector('#cautiousToggle button.active').dataset.val,
    rate_limit_kbps: parseInt(document.querySelector('#rateLimitToggle button.active').dataset.val),
    concurrency: parseInt(document.querySelector('#concurrencyToggle button.active').dataset.val),
    notifications_enabled: document.querySelector('#notifToggle button.active').dataset.val === 'on',
    sponsorblock_remove: document.getElementById('sponsorBtn').dataset.val === 'on',
    normalize_volume: document.querySelector('#normalizeToggle button.active').dataset.val === 'on',
    embed_thumbnail_metadata: document.querySelector('#embedToggle button.active').dataset.val === 'on',
    download_subtitles: document.querySelector('#subsToggle button.active').dataset.val === 'on',
    subtitle_langs: document.getElementById('subtitleLangs').value
  };
  fetch('/api/settings', {method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify(body)})
    .then(function(res){
      if (!res.ok) throw new Error('HTTP ' + res.status);
      return res.json();
    })
    .then(function(data){
      if (data && data.ok) {
        showToast('Configuración guardada correctamente.', 'ok');
        log('Configuracion guardada.');
      } else {
        showToast('La configuración no se pudo guardar.', 'error');
      }
    })
    .catch(function(err){
      showToast('La configuración no se ha guardado: ' + err.message, 'error');
      log('Error al guardar configuracion: ' + err.message);
    });
}

function loadVersion() {
  fetch('/api/ytdlp-version').then(function(res){ return res.json(); }).then(function(data){
    document.getElementById('ytdlpVersion').textContent = data.installed;
  });
}
function updateYtdlp() {
  const btn = document.getElementById('updateBtn');
  btn.disabled = true; btn.textContent = '⟳ Actualizando...';
  document.getElementById('updateLog').textContent = '';
  fetch('/api/ytdlp-update', {method:'POST'});
}
socket.on('update_log', function(d) { const el = document.getElementById('updateLog'); el.textContent += d.line + '\n'; el.scrollTop = el.scrollHeight; });
socket.on('update_done', function(d) {
  const btn = document.getElementById('updateBtn'); btn.disabled = false; btn.textContent = '⟳ Actualizar yt-dlp';
  log(d.message); loadVersion();
});

document.getElementById('url').addEventListener('keydown', function(e) {
  if (e.key === 'Enter') { e.preventDefault(); extract(); }
});
document.addEventListener('keydown', function(e) {
  if (e.ctrlKey && e.key === 'Enter') {
    e.preventDefault();
    if (document.getElementById('listCard').style.display !== 'none') startDownload();
  }
});

checkFfmpeg(); loadProfiles(); loadSettings(); loadVersion();
setInterval(function() { if (document.getElementById('tab-downloads').classList.contains('active')) { loadQueue(); } }, 3000);
