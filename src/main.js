const { invoke } = window.__TAURI__.core;
const { listen }  = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

// ─── Window controls ─────────────────────────────────────────────────────────
const appWindow = getCurrentWindow();
document.getElementById('win-minimize')?.addEventListener('click', () => appWindow.minimize());
document.getElementById('win-close')?.addEventListener('click', () => appWindow.close());

// ─── Version ──────────────────────────────────────────────────────────────────
invoke('get_version').then(v => {
  const el = document.getElementById('app-version');
  if (el) el.textContent = `v${v}`;
});

// ─── Profile (Static only) ────────────────────────────────────────────────────
invoke('set_profile', { profile: 'static' });

// ─── Output mode ──────────────────────────────────────────────────────────────
document.querySelectorAll('#output-btns .pill').forEach(btn => {
  btn.addEventListener('click', () => {
    const mode = btn.dataset.output;
    document.querySelectorAll('#output-btns .pill').forEach(b => b.classList.remove('active'));
    btn.classList.add('active');
    invoke('set_output_mode', { mode }).catch(() => {});
  });
});

// ─── State updates ────────────────────────────────────────────────────────────
function render(s) {
  // Connection
  const cs = document.getElementById('conn-status');
  if (cs) {
    cs.textContent = s.connected ? '● Connected' : '● Disconnected';
    cs.className = s.connected ? 'connected' : 'disconnected';
  }

  // Triggers
  const l2r = document.getElementById('l2-raw-bar');
  const r2r = document.getElementById('r2-raw-bar');
  const l2v = document.getElementById('l2-raw-val');
  const r2v = document.getElementById('r2-raw-val');
  if (l2r) l2r.style.width = `${(s.l2_raw / 255) * 100}%`;
  if (r2r) r2r.style.width = `${(s.r2_raw / 255) * 100}%`;
  if (l2v) l2v.textContent = s.l2_raw;
  if (r2v) r2v.textContent = s.r2_raw;

  // Overlay trigger fills
  const ovL2 = document.getElementById('ov-l2-raw');
  const ovR2 = document.getElementById('ov-r2-raw');
  if (ovL2) ovL2.setAttribute('height', s.l2_raw);
  if (ovR2) ovR2.setAttribute('height', s.r2_raw);
  const ovL2t = document.getElementById('ov-l2-val');
  const ovR2t = document.getElementById('ov-r2-val');
  if (ovL2t) ovL2t.textContent = s.l2_raw;
  if (ovR2t) ovR2t.textContent = s.r2_raw;

  // Stick dots
  const ls = document.getElementById('ov-ls-dot');
  const rs = document.getElementById('ov-rs-dot');
  if (ls) { ls.setAttribute('cx', 351.764 + (s.lx - 128) * 0.6); ls.setAttribute('cy', 528.548 - (s.ly - 128) * 0.4); }
  if (rs) { rs.setAttribute('cx', 763.456 + (s.rx - 128) * 0.6); rs.setAttribute('cy', 528.548 - (s.ry - 128) * 0.4); }

  // Error
  const em = document.getElementById('error-msg');
  if (em) em.textContent = s.error_msg || '';
}

listen('state-update', e => render(e.payload));

// ─── Load controller SVG ──────────────────────────────────────────────────────
async function loadControllerSVG() {
  try {
    const resp = await fetch('dualsense-base.svg');
    if (!resp.ok) return;
    const svg = await resp.text();
    const base = document.getElementById('ctrl-base');
    if (base) base.innerHTML = svg;
  } catch (_) {}
}

// ─── Init ─────────────────────────────────────────────────────────────────────
(async () => {
  await loadControllerSVG();
  const s = await invoke('get_state');
  document.querySelector('#output-btns [data-output="' + s.output_mode + '"]')?.classList.add('active');
  render(s);
})();
