const { invoke } = window.__TAURI__.core;
const { listen }  = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

// ─── Window controls (frameless) ────────────────────────────────────────────
const appWindow = getCurrentWindow();
document.getElementById('win-minimize')?.addEventListener('click', () => appWindow.minimize());
document.getElementById('win-close')?.addEventListener('click', () => appWindow.close());

invoke('get_version').then(v => {
  const el = document.getElementById('app-version');
  if (el) el.textContent = `v${v}`;
});

// Check for updates silently — shows banner if a newer version is available.
// Runs async on launch; network failure is silent (returns update_available: false).
invoke('check_update').then(({ update_available, latest }) => {
  if (!update_available) return;
  const banner = document.getElementById('update-banner');
  const ver    = document.getElementById('update-version');
  if (banner) banner.classList.remove('hidden');
  if (ver)    ver.textContent = `v${latest}`;
});

// ─── License / Edition ────────────────────────────────────────────────────────

const upgradeBanner    = document.getElementById('upgrade-banner');
const upgradeKeyRow    = document.getElementById('upgrade-key-row');
const keyInput         = document.getElementById('key-input');
const licenseSubmit    = document.getElementById('license-submit');
const licenseError     = document.getElementById('license-error');
const upgradeDismiss   = document.getElementById('upgrade-dismiss');
const upgradeExpandBtn = document.getElementById('upgrade-expand-btn');

// Dismiss hides the whole banner
upgradeDismiss.addEventListener('click', () => {
  upgradeBanner.classList.add('hidden');
});

// Upgrade → expands the key input row
upgradeExpandBtn.addEventListener('click', () => {
  upgradeKeyRow.classList.toggle('hidden');
  if (!upgradeKeyRow.classList.contains('hidden')) keyInput.focus();
});

let currentEdition = 'free';

function applyEdition(edition) {
  currentEdition = edition;
  const isFull = edition === 'full';

  // Show/hide upgrade banner
  upgradeBanner.classList.toggle('hidden', isFull);

  // Lock strength buttons 1-3 in Free tier
  document.querySelectorAll('#strength-btns .pill').forEach((btn, i) => {
    btn.classList.toggle('locked', !isFull && i > 0);
    btn.title = (!isFull && i > 0) ? 'Full Immersion only' : '';
  });

  // Lock shift feedback button
  const sb = document.getElementById('shift-btn');
  if (sb) { sb.classList.toggle('locked', !isFull); sb.title = !isFull ? 'Full Immersion only' : ''; }

  // Lock burst/full-auto weapons in Free tier (semi weapons stay available)
  document.querySelectorAll('#weapon-btns .pill').forEach(btn => {
    const free = FREE_WEAPONS.has(btn.dataset.weapon);
    btn.classList.toggle('locked', !isFull && !free);
    btn.title = (!isFull && !free) ? 'Full Immersion only' : '';
  });
}

// ─── Pro tier ($4) — gates the Lab ────────────────────────────────────────────
let currentPro = false;

function applyPro(pro) {
  currentPro = !!pro;
  const labBtn = document.getElementById('lab-toggle');
  if (labBtn) {
    labBtn.classList.toggle('pro-locked', !currentPro);
    labBtn.textContent = currentPro ? '🔬 Lab' : '🔬 Lab 🔒';
    labBtn.title = currentPro ? 'Lab — effect tester & racing feel' : 'The Lab is a Pro feature — $4 to unlock';
  }
  // If a non-Pro user somehow has the Lab open, close it.
  if (!currentPro) {
    const lp = document.getElementById('lab-panel');
    if (lp && !lp.classList.contains('hidden') && typeof closeLab === 'function') closeLab();
  }
}

async function initSession(key = null) {
  const result = await invoke('init_session', { key });
  applyEdition(result.edition ?? 'free');
  applyPro(result.pro);
  if (!result.ok && result.error) {
    licenseError.textContent = result.error;
  } else {
    licenseError.textContent = '';
  }
  return result;
}

licenseSubmit.addEventListener('click', async () => {
  const key = keyInput.value.trim();
  if (!key) return;
  licenseSubmit.disabled = true;
  licenseError.textContent = 'Activating...';
  await initSession(key);
  licenseSubmit.disabled = false;
});
keyInput.addEventListener('keydown', e => { if (e.key === 'Enter') licenseSubmit.click(); });

// ─── Sound system ─────────────────────────────────────────────────────────────

let _actx = null;
function actx() {
  if (!_actx) _actx = new (window.AudioContext || window.webkitAudioContext)();
  return _actx;
}
function beep(freq, dur, type = 'square', vol = 0.07) {
  try {
    const ctx = actx(), osc = ctx.createOscillator(), g = ctx.createGain();
    osc.type = type; osc.frequency.value = freq;
    g.gain.setValueAtTime(vol, ctx.currentTime);
    g.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + dur);
    osc.connect(g); g.connect(ctx.destination);
    osc.start(ctx.currentTime); osc.stop(ctx.currentTime + dur);
  } catch (_) {}
}
function sweep(f1, f2, dur, type = 'square', vol = 0.07) {
  try {
    const ctx = actx(), osc = ctx.createOscillator(), g = ctx.createGain();
    osc.type = type;
    osc.frequency.setValueAtTime(f1, ctx.currentTime);
    osc.frequency.linearRampToValueAtTime(f2, ctx.currentTime + dur);
    g.gain.setValueAtTime(vol, ctx.currentTime);
    g.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + dur);
    osc.connect(g); g.connect(ctx.destination);
    osc.start(ctx.currentTime); osc.stop(ctx.currentTime + dur);
  } catch (_) {}
}

const PROFILE_FREQ = { racing:440, static:466, gun:415, melee:494, audio:523 };
const STR_FREQ = [330, 440, 550, 660];
function soundProfile(p)  { beep(PROFILE_FREQ[p] || 440, 0.06, 'sine', 0.09); }
function soundStrength(i) { beep(STR_FREQ[i] || 440, 0.06, 'triangle', 0.09); }
function soundOn()  { sweep(440, 880, 0.07, 'square', 0.07); }
function soundOff() { sweep(880, 440, 0.07, 'square', 0.07); }
function soundView(){ beep(330, 0.09, 'sine', 0.06); }

// ─── State ────────────────────────────────────────────────────────────────────

let currentProfile     = 'racing';
let currentStrengthIdx = 2;
let viewMode           = 'full';

const PROFILES = ['racing', 'static', 'gun', 'melee', 'audio', 'minecraft'];
const STRENGTHS_COUNT = 4;

// ─── Element refs ─────────────────────────────────────────────────────────────

const appEl         = document.getElementById('app');
const connStatus    = document.getElementById('conn-status');
const telemStatus   = document.getElementById('telem-status');
const triggerSection = document.getElementById('trigger-section');
const profileBtns   = document.querySelectorAll('#profile-btns .pill');
const strengthBtns  = document.querySelectorAll('#strength-btns .pill');
const outputBtns    = document.querySelectorAll('#output-btns .pill');
const outputHint    = document.getElementById('output-hint');
const shiftControls = document.getElementById('shift-controls');
const gunControls   = document.getElementById('gun-controls');
const shiftBtn      = document.getElementById('shift-btn');
const weaponBtns    = document.querySelectorAll('#weapon-btns .pill');
const gameToggle    = document.getElementById('game-toggle');
const gameRow       = document.getElementById('game-row');
const gameSelect    = document.getElementById('game-select');
const di2Controls   = document.getElementById('di2-controls');
const di2WeaponBtns = document.querySelectorAll('#di2-controls .pill[data-di2]');
const GAME_LABELS   = { deadisland2: 'Dead Island 2', minecraft: 'Minecraft' };
// Which game entry is active under the Game tab: 'deadisland2' | 'minecraft' | null.
// Local UI state — DI2 rides the melee/gun engines, so the backend profile alone
// can't tell DI2 apart from the standalone Melee/Gun profiles.
let gameSelection = null;
// Weapons that fire as a single semi shot are available in the Free tier;
// the rest (burst / full-auto patterns) require Full Immersion.
const FREE_WEAPONS  = new Set(['pistol', 'revolver', 'rifle', 'shotgun', 'sniper']);
const shiftInfo     = document.getElementById('shift-info');
const errorMsg      = document.getElementById('error-msg');
const audioRow      = document.getElementById('audio-row');
const mcRow         = document.getElementById('mc-row');
const mcConn        = document.getElementById('mc-conn');
const mcItem        = document.getElementById('mc-item');
const mcAction      = document.getElementById('mc-action');
const viewToggleBtn = document.getElementById('view-toggle');
const keyHintsEl    = document.getElementById('key-hints');
const themeToggle   = document.getElementById('theme-toggle');

function applyTheme(theme) {
  const resolved = theme === 'light' ? 'light' : 'dark';
  appEl.dataset.theme = resolved;
  themeToggle.textContent = resolved === 'light' ? 'Dark' : 'Light';
  themeToggle.title = resolved === 'light'
    ? 'Switch to dark appearance'
    : 'Switch to light appearance';
  localStorage.setItem('dualsense-theme', resolved);
}

applyTheme(localStorage.getItem('dualsense-theme') || 'dark');
themeToggle.addEventListener('click', () => {
  applyTheme(appEl.dataset.theme === 'dark' ? 'light' : 'dark');
});

// Overlay SVG elements
const ovL2Raw  = document.getElementById('ov-l2-raw');
const ovR2Raw  = document.getElementById('ov-r2-raw');
const ovL2Out  = document.getElementById('ov-l2-out');
const ovR2Out  = document.getElementById('ov-r2-out');
const ovL2Val  = document.getElementById('ov-l2-val');
const ovR2Val  = document.getElementById('ov-r2-val');
const ovL2FVal = document.getElementById('ov-l2-fval');
const ovR2FVal = document.getElementById('ov-r2-fval');
const ovLsDot  = document.getElementById('ov-ls-dot');
const ovRsDot  = document.getElementById('ov-rs-dot');

// SVG coordinate constants (from the daidr dualsense.svg design file)
const L2_BOTTOM = 116.785, L2_HEIGHT = 110.916, L2_X = 138, L2_W = 114;
const R2_X      = 866;  // same height/width, mirrored
const LS_CX = 351.764, LS_CY = 528.548;   // left stick center
const RS_CX = 763.456, RS_CY = 528.548;   // right stick center
const STICK_TRAVEL = 38;                    // max SVG-coord radius for dot

// Button bitmask (buttons = byte8 | (byte9 << 8))
const BTN_SQUARE   = 1 << 4;
const BTN_CROSS    = 1 << 5;
const BTN_CIRCLE   = 1 << 6;
const BTN_TRIANGLE = 1 << 7;
const BTN_L1       = 1 << 8;
const BTN_R1       = 1 << 9;
const BTN_CREATE   = 1 << 12;
const BTN_OPTIONS  = 1 << 13;
const BTN_L3       = 1 << 14;
const BTN_R3       = 1 << 15;
const DPAD_MASK    = 0xF;

// ─── DualSense SVG loader ─────────────────────────────────────────────────────

const CTRL_COLOR   = '#30363d';   // default stroke (--border)
const CTRL_FILL    = '#8b949e';   // fill for small indicators (--muted)
const CTRL_INNER   = '#161b22';   // stick inner fill (--surface)
const BTN_COLORS   = { triangle:'#3fb950', circle:'#f85149', cross:'#58a6ff', rect:'#bf7dd8' };

let ctrlElems = {};   // id → SVG element reference

function getProfileColor() {
  const map = { racing:'#00c8ff', static:'#cc44ff', gun:'#ff4500', melee:'#ff8c00', audio:'#1a8fff' };
  return map[currentProfile] || '#00c8ff';
}

async function loadControllerSVG() {
  try {
    const resp = await fetch('./dualsense.svg');
    let text  = await resp.text();

    // Re-theme: red outlines → dark border; red fills → muted/dark
    text = text
      .replace(/stroke:#f00/g,  'stroke:' + CTRL_COLOR)
      .replace(/fill:#f00/g,    'fill:'   + CTRL_FILL)
      .replace(/fill:#900000/g, 'fill:'   + CTRL_INNER)
      .replace(/stroke-width:4px/g, 'stroke-width:1.5px');

    const parser = new DOMParser();
    const svgDoc = parser.parseFromString(text, 'image/svg+xml');
    const svg = svgDoc.documentElement;
    svg.setAttribute('width', '100%');
    svg.setAttribute('height', '100%');

    const base = document.getElementById('ctrl-base');
    base.innerHTML = '';
    base.appendChild(svg);

    // Collect interactive elements by their original SVG IDs
    [
      'l2','r2','l1','r1',
      'triangle','cross','circle','rect',
      'dpad-up','dpad-down','dpad-left','dpad-right',
      'touchpad','options','create',
      'l3','r3','l3-border','r3-border',
      't1','t2',
    ].forEach(id => {
      ctrlElems[id] = document.getElementById(id);
    });

    // Tag dpad children ONCE at load time from original SVG style (before any JS mutation).
    // getAttribute('style') returns the source text here — no browser normalization yet.
    // Later calls to ctrlActive use data-has-fill to avoid the fill:none vs "fill: none" mismatch.
    ['dpad-up','dpad-down','dpad-left','dpad-right'].forEach(id => {
      const g = ctrlElems[id];
      if (!g) return;
      g.querySelectorAll('path, rect').forEach(ch => {
        const s = ch.getAttribute('style') || '';
        // "fill:none" = outline-only path; "fill:#color" = filled arrow
        ch.dataset.hasFill = (s.includes('fill:') && !s.includes('fill:none')) ? '1' : '0';
      });
    });
  } catch (e) {
    console.warn('Controller SVG load failed:', e);
  }
}

// Highlight/dim a controller element
function ctrlActive(id, on) {
  const el = ctrlElems[id];
  if (!el) return;
  const prof = getProfileColor();

  if (id.startsWith('dpad')) {
    // dpad elements are <g> groups with two children:
    //   - outer path (fill:none, stroke-only)  → tagged data-has-fill="0"
    //   - inner arrow (filled)                  → tagged data-has-fill="1"
    // We use the tag set at SVG load time so browser style normalization
    // ("fill: none" vs "fill:none") never causes the wrong branch.
    el.querySelectorAll('path, rect').forEach(ch => {
      if (ch.dataset.hasFill === '1') {
        ch.style.fill   = on ? prof : CTRL_FILL;
        ch.style.stroke = 'none';
      } else {
        ch.style.stroke      = on ? prof : CTRL_COLOR;
        ch.style.strokeWidth = on ? '2.5px' : '1.5px';
        ch.style.fill        = 'none';
      }
    });
    return;
  }

  el.style.stroke      = on ? (BTN_COLORS[id] || prof) : CTRL_COLOR;
  el.style.strokeWidth = on ? '2.5px' : '1.5px';
}

// ─── Controller overlay updater ───────────────────────────────────────────────

function updateController(s) {
  // Trigger fills: height grows upward from L2_BOTTOM
  const l2H = (s.l2_raw   / 255) * L2_HEIGHT;
  const l2O = (s.l2_force / 255) * L2_HEIGHT;
  const r2H = (s.r2_raw   / 255) * L2_HEIGHT;
  const r2O = (s.r2_force / 255) * L2_HEIGHT;

  ovL2Raw.setAttribute('y',      (L2_BOTTOM - l2H).toFixed(2));
  ovL2Raw.setAttribute('height', l2H.toFixed(2));
  ovL2Out.setAttribute('y',      (L2_BOTTOM - l2O).toFixed(2));
  ovL2Out.setAttribute('height', l2O.toFixed(2));

  ovR2Raw.setAttribute('y',      (L2_BOTTOM - r2H).toFixed(2));
  ovR2Raw.setAttribute('height', r2H.toFixed(2));
  ovR2Out.setAttribute('y',      (L2_BOTTOM - r2O).toFixed(2));
  ovR2Out.setAttribute('height', r2O.toFixed(2));

  // Value labels
  ovL2Val.textContent  = s.l2_raw;
  ovL2FVal.textContent = '⇒' + s.l2_force;
  ovR2Val.textContent  = s.r2_raw;
  ovR2FVal.textContent = s.r2_force + '⇐';

  // Stick dots
  const lx = s.lx !== undefined ? s.lx : 128;
  const ly = s.ly !== undefined ? s.ly : 128;
  const rx = s.rx !== undefined ? s.rx : 128;
  const ry = s.ry !== undefined ? s.ry : 128;

  ovLsDot.setAttribute('cx', (LS_CX + (lx - 128) / 128 * STICK_TRAVEL).toFixed(2));
  ovLsDot.setAttribute('cy', (LS_CY + (ly - 128) / 128 * STICK_TRAVEL).toFixed(2));
  ovRsDot.setAttribute('cx', (RS_CX + (rx - 128) / 128 * STICK_TRAVEL).toFixed(2));
  ovRsDot.setAttribute('cy', (RS_CY + (ry - 128) / 128 * STICK_TRAVEL).toFixed(2));

  // Button highlights
  const b    = s.buttons !== undefined ? s.buttons : 0;
  const dpad = b & DPAD_MASK;

  ctrlActive('triangle',   !!(b & BTN_TRIANGLE));
  ctrlActive('rect',       !!(b & BTN_SQUARE));
  ctrlActive('cross',      !!(b & BTN_CROSS));
  ctrlActive('circle',     !!(b & BTN_CIRCLE));
  ctrlActive('l1',         !!(b & BTN_L1));
  ctrlActive('r1',         !!(b & BTN_R1));
  ctrlActive('create',     !!(b & BTN_CREATE));
  ctrlActive('options',    !!(b & BTN_OPTIONS));

  ctrlActive('dpad-up',    dpad === 0 || dpad === 1 || dpad === 7);
  ctrlActive('dpad-right', dpad === 1 || dpad === 2 || dpad === 3);
  ctrlActive('dpad-down',  dpad === 3 || dpad === 4 || dpad === 5);
  ctrlActive('dpad-left',  dpad === 5 || dpad === 6 || dpad === 7);

  // L3/R3 press highlight (change inner fill)
  const l3  = ctrlElems['l3'];
  const r3  = ctrlElems['r3'];
  const prof = getProfileColor();
  if (l3) l3.style.fill = (b & BTN_L3) ? prof : CTRL_INNER;
  if (r3) r3.style.fill = (b & BTN_R3) ? prof : CTRL_INNER;

  // Touchpad button press
  ctrlActive('touchpad', !!s.touchpad_btn);

  // t1/t2 indicator dots: light up when finger is on touchpad, brighter when clicked
  const t1 = ctrlElems['t1'], t2 = ctrlElems['t2'];
  if (t1) t1.style.fill = s.touch0_active ? prof : CTRL_FILL;
  if (t2) t2.style.fill = s.touchpad_btn  ? prof : CTRL_FILL;

  // Finger touch tracking dot on touchpad
  // Touchpad SVG bounds: left≈332, right≈786, top≈143, bottom≈397
  const touchDot = document.getElementById('ov-touch-dot');
  if (touchDot) {
    if (s.touch0_active) {
      const tx = (332.5 + (s.touch0_x / 1919) * 453.1).toFixed(1);
      const ty = (143   + (s.touch0_y / 1079) * 254  ).toFixed(1);
      touchDot.setAttribute('cx', tx);
      touchDot.setAttribute('cy', ty);
      touchDot.setAttribute('visibility', 'visible');
    } else {
      touchDot.setAttribute('visibility', 'hidden');
    }
  }
}

// ─── View toggle ──────────────────────────────────────────────────────────────

function applyViewMode() {
  appEl.classList.toggle('compact', viewMode === 'compact');
  viewToggleBtn.textContent = viewMode === 'full' ? 'Compact' : 'Full';
}
async function toggleView() {
  soundView();
  viewMode = viewMode === 'full' ? 'compact' : 'full';
  applyViewMode();
  // Resize the native window to match the view mode
  const [w, h] = viewMode === 'compact' ? [560, 330] : [820, 540];
  await invoke('set_window_size', { width: w, height: h });
}
viewToggleBtn.addEventListener('click', toggleView);

// ─── Profile buttons ──────────────────────────────────────────────────────────

profileBtns.forEach(btn => {
  btn.addEventListener('click', async () => {
    const p = btn.dataset.profile;
    gameSelection = null;   // leaving the Game tab for a plain profile
    soundProfile(p);
    await invoke('set_profile', { profile: p });
  });
});

// ─── Strength buttons ─────────────────────────────────────────────────────────

strengthBtns.forEach(btn => {
  btn.addEventListener('click', async () => {
    if (btn.classList.contains('locked')) return;
    const idx = parseInt(btn.dataset.idx, 10);
    soundStrength(idx);
    await invoke('set_strength', { idx });
  });
});

// ─── Output mode (DualSense / Xbox) ───────────────────────────────────────────

outputBtns.forEach(btn => btn.addEventListener('click', async () => {
  await invoke('set_output_mode', { mode: btn.dataset.output });
  soundOn();
}));

// ─── Game rumble passthrough (Xbox output) ────────────────────────────────────

const ptRow       = document.getElementById('pt-row');
const ptEnable    = document.getElementById('pt-enable');
const ptIntensity = document.getElementById('pt-intensity');
const ptKick      = document.getElementById('pt-kick');
const ptLb        = document.getElementById('pt-lb');
const ptMeterFill = document.getElementById('pt-meter-fill');

const ptCfg = { enabled: true, intensity: 1.0, trigger_kick: true, lightbar: true };

const pushPt = () => invoke('set_rumble_passthrough', { cfg: { ...ptCfg } }).catch(() => {});

function syncPtUI() {
  ptEnable.classList.toggle('on', ptCfg.enabled);
  ptEnable.classList.toggle('off', !ptCfg.enabled);
  ptIntensity.value = Math.round(ptCfg.intensity * 100);
  document.getElementById('pt-intensity-val').textContent = `${ptCfg.intensity.toFixed(1)}×`;
  ptKick.classList.toggle('on', ptCfg.trigger_kick);
  ptLb.classList.toggle('on', ptCfg.lightbar);
}

function initPtFromState(s) {
  ptCfg.enabled      = !!s.pt_enabled;
  ptCfg.intensity    = s.pt_intensity ?? 1.0;
  ptCfg.trigger_kick = !!s.pt_trigger_kick;
  ptCfg.lightbar     = !!s.pt_lightbar;
  syncPtUI();
}

ptEnable.addEventListener('click', () => { ptCfg.enabled = !ptCfg.enabled; syncPtUI(); pushPt(); });
ptKick.addEventListener('click',   () => { ptCfg.trigger_kick = !ptCfg.trigger_kick; syncPtUI(); pushPt(); });
ptLb.addEventListener('click',     () => { ptCfg.lightbar = !ptCfg.lightbar; syncPtUI(); pushPt(); });
ptIntensity.addEventListener('input', () => { ptCfg.intensity = +ptIntensity.value / 100; syncPtUI(); pushPt(); });

// ─── Audio haptic EQ ──────────────────────────────────────────────────────────

const audioEq   = document.getElementById('audio-eq');
const eqSub     = document.getElementById('eq-sub');
const eqEng     = document.getElementById('eq-eng');
const eqGate    = document.getElementById('eq-gate');
const eqSubMeter = document.getElementById('eq-sub-meter');
const eqEngMeter = document.getElementById('eq-eng-meter');
const eqModeHint = document.getElementById('eq-mode-hint');

const eqCfg = { sub: 1.4, engine: 1.6, gate: 0.012 };
const pushEq = () => invoke('set_audio_tune', { cfg: { ...eqCfg } }).catch(() => {});

function syncEqUI() {
  eqSub.value = Math.round(eqCfg.sub * 100);
  document.getElementById('eq-sub-val').textContent = `${eqCfg.sub.toFixed(1)}×`;
  eqEng.value = Math.round(eqCfg.engine * 100);
  document.getElementById('eq-eng-val').textContent = `${eqCfg.engine.toFixed(1)}×`;
  eqGate.value = Math.round(eqCfg.gate * 1000);
  document.getElementById('eq-gate-val').textContent = eqCfg.gate.toFixed(3);
}

function initEqFromState(s) {
  eqCfg.sub    = s.audio_sub ?? 1.4;
  eqCfg.engine = s.audio_engine ?? 1.6;
  eqCfg.gate   = s.audio_gate ?? 0.012;
  syncEqUI();
}

eqSub.addEventListener('input',  () => { eqCfg.sub = +eqSub.value / 100; syncEqUI(); pushEq(); });
eqEng.addEventListener('input',  () => { eqCfg.engine = +eqEng.value / 100; syncEqUI(); pushEq(); });
eqGate.addEventListener('input', () => { eqCfg.gate = +eqGate.value / 1000; syncEqUI(); pushEq(); });

// ─── Shift toggle ─────────────────────────────────────────────────────────────

shiftBtn.addEventListener('click', async () => {
  if (shiftBtn.classList.contains('locked')) return;
  const on = await invoke('toggle_shift');
  on ? soundOn() : soundOff();
});

// ─── Weapon selection ─────────────────────────────────────────────────────────

weaponBtns.forEach(btn => btn.addEventListener('click', async () => {
  if (btn.classList.contains('locked')) return;
  await invoke('set_gun_weapon', { key: btn.dataset.weapon });
  soundOn();
}));

// ─── Game tab (per-game profiles, keeps the Profile row clean) ────────────────

gameToggle.addEventListener('click', () => {
  const show = gameRow.style.display === 'none';
  gameRow.style.display = show ? '' : 'none';
  soundOn();
});

gameSelect.addEventListener('change', async () => {
  const game = gameSelect.value;
  gameSelection = game || null;
  if (!game) return;
  // Dead Island 2 uses the melee engine by default (DI2-tuned feels.json); equipping a
  // firearm flips it to the gun engine. Minecraft is mod-driven.
  const profile = game === 'minecraft' ? 'minecraft' : 'melee';
  soundProfile(profile);
  await invoke('set_profile', { profile });
});

// ─── DI2 equipped-weapon selection (fists → guns) ─────────────────────────────
// data-di2 is "engine:key". Melee weapons run the melee swing engine; firearms run
// the gun recoil engine. Either way the active game stays Dead Island 2.

di2WeaponBtns.forEach(btn => btn.addEventListener('click', async () => {
  const [engine, key] = btn.dataset.di2.split(':');
  gameSelection = 'deadisland2';
  if (engine === 'gun') {
    await invoke('set_profile', { profile: 'gun' });
    await invoke('set_gun_weapon', { key });
  } else {
    await invoke('set_profile', { profile: 'melee' });
    await invoke('set_melee_weapon', { key });
  }
  soundOn();
}));

// ─── Keyboard shortcuts (original terminal keybinds) ──────────────────────────
// M → cycle profile  |  S → cycle strength  |  F → shift toggle  |  V → view toggle
// (Gun weapon is button-only — no keybind.)

async function cycleProfile() {
  const next = PROFILES[(PROFILES.indexOf(currentProfile) + 1) % PROFILES.length];
  soundProfile(next);
  await invoke('set_profile', { profile: next });
}

async function cycleStrength() {
  const next = (currentStrengthIdx + 1) % STRENGTHS_COUNT;
  soundStrength(next);
  await invoke('set_strength', { idx: next });
}

document.addEventListener('keydown', async (e) => {
  if (e.target.tagName === 'INPUT') return;
  if (e.metaKey || e.ctrlKey || e.altKey) return;

  switch (e.key.toLowerCase()) {
    case 'm': await cycleProfile(); break;
    case 's': await cycleStrength(); break;
    case 'f': {
      const on = await invoke('toggle_shift');
      on ? soundOn() : soundOff();
      break;
    }
    case 'v': await toggleView(); break;
  }
});

// ─── Render ───────────────────────────────────────────────────────────────────

function setBar(id, pct) {
  document.getElementById(id).style.width = (pct * 100).toFixed(1) + '%';
}

function render(s) {
  // Edition (re-apply so UI stays in sync with Rust state)
  if (s.edition && s.edition !== currentEdition) applyEdition(s.edition);
  if (typeof s.pro === 'boolean' && s.pro !== currentPro) applyPro(s.pro);

  // Connection
  connStatus.textContent = s.connected ? '⬤ Connected' : '⬤ Disconnected';
  connStatus.className   = s.connected ? 'connected' : 'disconnected';

  // Forza telemetry indicator is set below (the isRacing-aware three-state block),
  // which is the single source of truth — no duplicate write here.

  // Profile
  currentProfile = s.profile;
  // Keep gameSelection coherent with the live profile (backend restored a profile, or
  // another control changed it). DI2 spans the melee + gun engines, so only clear the
  // game selection when the profile leaves both.
  if (s.profile === 'minecraft') gameSelection = 'minecraft';
  else if (s.profile !== 'melee' && s.profile !== 'gun') gameSelection = null;
  const di2Active = gameSelection === 'deadisland2' &&
                    (s.profile === 'melee' || s.profile === 'gun');
  profileBtns.forEach(btn => {
    // Don't light the Melee/Gun pill when DI2 (which rides those engines) is active.
    const p = btn.dataset.profile;
    const match = p === s.profile && !((p === 'melee' || p === 'gun') && di2Active);
    btn.className = 'pill' + (match ? ` active-${s.profile}` : '');
  });
  // Game pill: active + named when a per-game entry is live.
  const gameActive = gameSelection === 'minecraft' || di2Active;
  gameToggle.className = 'pill' + (gameActive ? ' active-melee' : '');
  gameToggle.textContent = gameActive ? `Game: ${GAME_LABELS[gameSelection]}` : 'Game ▾';
  gameSelect.value = gameActive ? gameSelection : '';
  triggerSection.dataset.profile = s.profile;
  appEl.dataset.profile = s.profile;

  // Output mode
  const outMode = s.output_mode || 'dualsense';
  outputBtns.forEach(btn => {
    btn.classList.toggle('active-gun', btn.dataset.output === outMode);
  });
  outputHint.textContent = outMode === 'xbox'
    ? 'Game reads a virtual Xbox pad. Haptics still on the DualSense. Needs ViGEmBus + HidHide (Windows).'
    : '';

  // Game rumble passthrough row — Xbox mode only; meter shows live game rumble.
  ptRow.style.display = outMode === 'xbox' ? '' : 'none';
  if (outMode === 'xbox') {
    const lvl = Math.max(s.game_rl || 0, s.game_rr || 0) / 255;
    ptMeterFill.style.width = `${Math.round(lvl * 100)}%`;
  }

  // Strength
  currentStrengthIdx = s.strength_idx;
  strengthBtns.forEach(btn => {
    const idx = parseInt(btn.dataset.idx, 10);
    btn.className = 'pill' + (idx === s.strength_idx ? ` str-active-${s.strength_idx}` : '');
  });

  // Compact trigger bars
  setBar('l2-raw-bar', s.l2_raw   / 255);
  setBar('r2-raw-bar', s.r2_raw   / 255);
  setBar('l2-out-bar', s.l2_force / 255);
  setBar('r2-out-bar', s.r2_force / 255);
  document.getElementById('l2-raw-val').textContent = s.l2_raw;
  document.getElementById('r2-raw-val').textContent = s.r2_raw;
  document.getElementById('l2-out-val').textContent = s.l2_force;
  document.getElementById('r2-out-val').textContent = s.r2_force;

  // Controller SVG overlay
  updateController(s);

  // Profile-specific UI
  const isRacing = s.profile === 'racing';
  const isGun    = s.profile === 'gun';
  const isAudio  = s.profile === 'audio';
  const isMinecraft = s.profile === 'minecraft';

  shiftControls.style.display = isRacing ? '' : 'none';
  // Standalone Gun controls hide while DI2 is active (DI2 has its own combined selector).
  gunControls.classList.toggle('hidden', !isGun || di2Active);
  di2Controls.classList.toggle('hidden', !di2Active);
  audioRow.classList.toggle('visible', isAudio);
  if (audioEq) audioEq.classList.toggle('visible', isAudio);
  if (isAudio) {
    const lbl = audioRow.querySelector('.audio-label');
    if (lbl) lbl.textContent = s.audio_true
      ? 'System audio — TRUE HAPTICS (USB)'
      : 'System audio — Energy';
    if (eqSubMeter) eqSubMeter.style.width = `${Math.round((s.audio_sub_level || 0) * 100)}%`;
    if (eqEngMeter) eqEngMeter.style.width = `${Math.round((s.audio_eng_level || 0) * 100)}%`;
    if (eqModeHint) eqModeHint.textContent = s.audio_true ? 'true haptics (USB)' : 'reactive rumble';
  }
  mcRow.classList.toggle('visible', isMinecraft);

  // Highlight the equipped DI2 weapon (engine:key matches the live profile + weapon)
  const di2Engine = isGun ? 'gun' : 'melee';
  const di2Key    = isGun ? (s.gun_weapon || 'pistol') : (s.melee_weapon || 'fists');
  di2WeaponBtns.forEach(btn => {
    btn.classList.toggle('active-melee', btn.dataset.di2 === `${di2Engine}:${di2Key}`);
  });

  if (isMinecraft) {
    mcConn.textContent = s.mc_connected ? '⬤ Mod connected' : '⬤ Mod not connected';
    mcConn.className   = 'mc-conn' + (s.mc_connected ? ' on' : '');
    mcItem.textContent = s.mc_connected ? (s.mc_item || 'empty') : '—';
    let action = '';
    if (s.mc_connected) {
      if (s.mc_mining)        action = 'Mining';
      else if (s.mc_blocking) action = 'Blocking';
      else if (s.mc_using)    action = 'Using';
    }
    mcAction.textContent = action;
    mcAction.style.display = action ? '' : 'none';
  }

  shiftBtn.className   = `toggle-pill ${s.shift_enabled ? 'on' : 'off'}`;
  shiftBtn.textContent = `Shift Feedback${s.shift_enabled ? '' : ' (off)'}`;

  // Highlight the selected weapon
  const weapon = s.gun_weapon || 'pistol';
  weaponBtns.forEach(btn => {
    const sel = btn.dataset.weapon === weapon;
    btn.classList.toggle('active-gun', sel);
  });

  if (isAudio) setBar('audio-bar', s.audio_pct);

  shiftInfo.textContent = (isRacing && s.shift_count > 0)
    ? `Shift: ${s.last_shift_dir}  ×${s.shift_count}` : '';

  // Forza telemetry indicator — only relevant on the Racing profile. Three HONEST
  // states, driven by real packet reception (no placebo):
  //   • not connected  — nothing is arriving on any port (game closed / Data Out off
  //                       / wrong port). Dim.
  //   • connected      — packets ARE arriving but IsRaceOn=0 (paused / in a menu).
  //                       This is the proof the Data Out link works. Amber.
  //   • live + gear     — packets arriving AND racing. Green, with the live gear.
  if (telemStatus) {
    if (!isRacing) {
      telemStatus.style.display = 'none';
    } else {
      telemStatus.style.display = '';
      if (s.telem_on) {
        const g = s.telem_gear > 0 ? `  ·  Gear ${s.telem_gear}` : '';
        telemStatus.textContent = `⬤ Forza connected · live${g}`;
        telemStatus.className = 'telem-on';
      } else if (s.telem_connected) {
        telemStatus.textContent = '⬤ Forza connected · paused';
        telemStatus.className = 'telem-paused';
      } else {
        telemStatus.textContent = '○ Forza not detected';
        telemStatus.className = 'telem-off';
      }
    }
  }

  errorMsg.textContent = s.error_msg || '';

  keyHintsEl.textContent = s.error_msg ? '' : 'M Profile  S Strength  F Shift  G Gun  V View';

  // Motion panel live visualization (only when open)
  if (motionPanel && !motionPanel.classList.contains('hidden')) updateMotionViz(s);

  // Live tach in the Racing Lab (only when the Lab is open on the racing tab)
  if (!labPanel.classList.contains('hidden') && labActiveTab === 'racing') updateTach(s);
}

// ─── State updates ────────────────────────────────────────────────────────────

listen('state-update', e => render(e.payload));

// ─── Init ─────────────────────────────────────────────────────────────────────

(async () => {
  await initSession(null);
  await loadControllerSVG();
  applyViewMode();
  const s = await invoke('get_state');
  initPtFromState(s);
  initEqFromState(s);
  initDrivetrainFromState(s);
  initDtProfileFromState(s);
  render(s);
})();

// ─── Trigger Lab ────────────────────────────────────────────────────────────
// Live test bench for adaptive-trigger effect modes. Bypasses profile logic in
// Rust (set_test) so you can feel any mode + params and decide what to keep.
const MODE_INFO = {
  0:  { params: [], hint: 'Off — no resistance.' },
  1:  { params: ['Start pos', 'Force'],
        hint: 'Rigid: constant resistance from the start position to the bottom. A static wall — does NOT kick a pulled trigger.' },
  2:  { params: ['Start pos', 'End pos', 'Force'],
        hint: 'Weapon: resistance up to the break point, then snaps free. The classic gun "click". No kick once broken.' },
  6:  { params: ['Frequency', 'Amplitude', 'Start pos'],
        hint: 'Vibration: hammers the trigger motor at a frequency. Actively shoves your finger — the one that gives a felt recoil kick. Try freq 20-60, amp 200-255.' },
  33: { params: ['Zone mask lo', 'Zone mask hi', 'Force A', 'Force B', 'Force C'],
        hint: 'Feedback (zoned): per-region resistance across 10 zones. Raw bytes — experiment.' },
  37: { params: ['Zone mask lo', 'Zone mask hi', 'Force'],
        hint: 'Weapon (zoned): multi-zone break. Raw bytes.' },
  38: { params: ['Zone mask lo', 'Zone mask hi', 'Amp A', 'Amp B', 'Freq'],
        hint: 'Vibration (zoned): vibration assigned to specific travel zones. Raw bytes.' },
  35: { params: ['Start', 'End', '1st foot', '2nd foot', 'Freq'],
        hint: 'Galloping: rhythmic two-foot clop-clop oscillation.' },
  39: { params: ['Start', 'End', 'Amp A', 'Amp B', 'Freq', 'Period'],
        hint: 'Machine: sustained dual-frequency rumble in the trigger. Meatier than plain vibration.' },
};

const labToggle   = document.getElementById('lab-toggle');
const labPanel    = document.getElementById('lab-panel');
const labClose    = document.getElementById('lab-close');
const labTabs     = document.querySelectorAll('#lab-tabs .lab-tab');
const labTabContents = document.querySelectorAll('#lab-panel .lab-tab-content');
let   labActiveTab = 'racing';
const labModeSel   = document.getElementById('lab-mode');
const labHint      = document.getElementById('lab-hint');
const labParamsEl  = document.getElementById('lab-params');
const labSideBtns  = document.querySelectorAll('#lab-side-btns .pill');
const labRl        = document.getElementById('lab-rl');
const labRr        = document.getElementById('lab-rr');
const labRlVal     = document.getElementById('lab-rl-val');
const labRrVal     = document.getElementById('lab-rr-val');
const labHoldBtn   = document.getElementById('lab-hold');
const labPulseBtn  = document.getElementById('lab-pulse');
const labOffBtn    = document.getElementById('lab-off');

const labState = { side: 1, mode: 6, params: new Array(10).fill(0), rumbleL: 0, rumbleR: 0, holding: false };

function labSend(active) {
  invoke('set_test', { effect: {
    active,
    side:     labState.side,
    mode:     labState.mode,
    params:   labState.params.slice(0, 10),
    rumble_l: labState.rumbleL,
    rumble_r: labState.rumbleR,
  }});
}

function labRebuildParams() {
  const info = MODE_INFO[labState.mode] || { params: [], hint: '' };
  labHint.textContent = info.hint;
  labParamsEl.innerHTML = '';
  info.params.forEach((label, i) => {
    const row = document.createElement('div');
    row.className = 'lab-param';
    const lbl = document.createElement('span');
    lbl.className = 'lab-param-label';
    lbl.textContent = label;
    const slider = document.createElement('input');
    slider.type = 'range'; slider.min = '0'; slider.max = '255';
    slider.value = String(labState.params[i] || 0);
    const val = document.createElement('span');
    val.className = 'slider-val';
    val.textContent = slider.value;
    slider.addEventListener('input', () => {
      labState.params[i] = parseInt(slider.value, 10);
      val.textContent = slider.value;
      if (labState.holding) labSend(true);
    });
    row.append(lbl, slider, val);
    labParamsEl.appendChild(row);
  });
}

labSideBtns.forEach(btn => btn.addEventListener('click', () => {
  labState.side = parseInt(btn.dataset.side, 10);
  labSideBtns.forEach(b => b.classList.toggle('active-gun', b === btn));
  if (labState.holding) labSend(true);
}));

labModeSel.addEventListener('change', () => {
  labState.mode = parseInt(labModeSel.value, 10);
  labState.params = new Array(10).fill(0);
  labRebuildParams();
  if (labState.holding) labSend(true);
});

labRl.addEventListener('input', () => {
  labState.rumbleL = parseInt(labRl.value, 10);
  labRlVal.textContent = labRl.value;
  if (labState.holding) labSend(true);
});
labRr.addEventListener('input', () => {
  labState.rumbleR = parseInt(labRr.value, 10);
  labRrVal.textContent = labRr.value;
  if (labState.holding) labSend(true);
});

labHoldBtn.addEventListener('click', () => {
  labState.holding = !labState.holding;
  labHoldBtn.classList.toggle('on', labState.holding);
  labSend(labState.holding);
});

labPulseBtn.addEventListener('click', () => {
  labSend(true);
  setTimeout(() => { if (!labState.holding) labSend(false); }, 150);
});

function labStop() {
  labState.holding = false;
  labHoldBtn.classList.remove('on');
  labSend(false);
}
labOffBtn.addEventListener('click', labStop);

// ─── Gun presets — one-tap feels ────────────────────────────────────────────
// action: 'pulse' = single recoil snap | 'burst3' = three quick snaps |
//         'hold' = sustained (auto fire / pull-through wall)
const LAB_PRESETS = [
  { label: 'Pistol',   sub: 'crisp snap',     mode: 6, params: [35, 230, 0], rl: 150, rr: 90,  action: 'pulse'  },
  { label: 'Rifle',    sub: 'semi punch',     mode: 6, params: [40, 255, 0], rl: 160, rr: 110, action: 'pulse'  },
  { label: 'Burst',    sub: '3-round',        mode: 6, params: [30, 255, 0], rl: 180, rr: 200, action: 'burst3' },
  { label: 'AR Auto',  sub: '~600rpm chug',   mode: 6, params: [10, 255, 0], rl: 130, rr: 180, action: 'hold'   },
  { label: 'SMG',      sub: 'fast buzz',      mode: 6, params: [16, 200, 0], rl: 90,  rr: 210, action: 'hold'   },
  { label: 'LMG',      sub: 'heavy chug',     mode: 6, params: [7,  255, 0], rl: 210, rr: 220, action: 'hold'   },
  { label: 'Shotgun',  sub: 'big thump',      mode: 6, params: [8,  255, 0], rl: 255, rr: 160, action: 'pulse'  },
  { label: 'Sniper',   sub: 'hard snap',      mode: 6, params: [20, 255, 0], rl: 255, rr: 120, action: 'pulse'  },
  { label: 'Machine',  sub: 'dual-freq auto', mode: 39, params: [0, 255, 200, 120, 8, 4], rl: 160, rr: 200, action: 'hold' },
  { label: 'Wall',     sub: 'pull to feel',   mode: 2, params: [70, 100, 200], rl: 0, rr: 0, action: 'hold' },
];

function labApplyPreset(p) {
  // Stop any current effect first so holds don't stack.
  labStop();
  labState.side    = 1; // R2
  labState.mode    = p.mode;
  labState.params  = [...p.params, ...new Array(10).fill(0)].slice(0, 10);
  labState.rumbleL = p.rl;
  labState.rumbleR = p.rr;

  // Sync the manual controls so the user can fine-tune from here.
  labModeSel.value = String(p.mode);
  labSideBtns.forEach(b => b.classList.toggle('active-gun', parseInt(b.dataset.side, 10) === 1));
  labRl.value = String(p.rl); labRlVal.textContent = p.rl;
  labRr.value = String(p.rr); labRrVal.textContent = p.rr;
  labRebuildParams();

  if (p.action === 'hold') {
    labState.holding = true;
    labHoldBtn.classList.add('on');
    labSend(true);
  } else if (p.action === 'burst3') {
    let n = 0;
    const fire = () => {
      labSend(true);
      setTimeout(() => labSend(false), 70);
      if (++n < 3) setTimeout(fire, 110);
    };
    fire();
  } else { // pulse
    labSend(true);
    setTimeout(() => { if (!labState.holding) labSend(false); }, 160);
  }
}

const labPresetsEl = document.getElementById('lab-presets');
LAB_PRESETS.forEach(p => {
  const btn = document.createElement('button');
  btn.className = 'lab-preset';
  btn.innerHTML = `${p.label}<span class="lab-preset-sub">${p.sub}</span>`;
  btn.addEventListener('click', () => labApplyPreset(p));
  labPresetsEl.appendChild(btn);
});

// ─── Preview presets (Melee / Minecraft / Audio tabs) ───────────────────────
// Lightweight facsimiles that drive the raw effect injector (set_test) so each
// per-mode feel can be tested without the real game/input. side: 1=R2 0=L2 2=both.
// action: pulse | hold | burst3 | double. dur = pulse length ms.
function firePreset(p) {
  labStop(); // clear anything already running
  const side   = p.side ?? 1;
  const mode   = p.mode ?? 6;
  const params = [...(p.params || []), ...new Array(10).fill(0)].slice(0, 10);
  const rl = p.rl || 0, rr = p.rr || 0;
  const send = (active) =>
    invoke('set_test', { effect: { active, side, mode, params, rumble_l: rl, rumble_r: rr } });

  const act = p.action || 'pulse';
  if (act === 'hold') {
    labState.holding = true;          // so labStop() knows to clear it
    labState.side = side; labState.mode = mode; labState.params = params;
    send(true);
  } else if (act === 'burst3') {
    let n = 0;
    const fire = () => {
      send(true);
      setTimeout(() => send(false), 70);
      if (++n < 3) setTimeout(fire, 110);
    };
    fire();
  } else if (act === 'double') {
    send(true);
    setTimeout(() => send(false), 60);
    setTimeout(() => { send(true); setTimeout(() => send(false), 70); }, 200);
  } else { // pulse
    send(true);
    setTimeout(() => { if (!labState.holding) send(false); }, p.dur || 160);
  }
}

// Build a preset grid into the given container.
function buildPresetGrid(containerId, presets) {
  const el = document.getElementById(containerId);
  if (!el) return;
  presets.forEach(p => {
    const btn = document.createElement('button');
    btn.className = 'lab-preset';
    btn.innerHTML = `${p.label}<span class="lab-preset-sub">${p.sub}</span>`;
    btn.addEventListener('click', () => firePreset(p));
    el.appendChild(btn);
  });
}

// Dead Island 2 melee weapon types — swing-connect impacts.
const MELEE_PRESETS = [
  { label: 'Knife',       sub: 'quick stab',   mode: 6, params: [40, 200, 0], rl: 90,  rr: 150, action: 'pulse', dur: 90  },
  { label: 'Machete',     sub: 'bladed slash', mode: 6, params: [30, 230, 0], rl: 150, rr: 130, action: 'pulse', dur: 120 },
  { label: 'Cleaver',     sub: 'heavy chop',   mode: 6, params: [22, 245, 0], rl: 190, rr: 120, action: 'pulse', dur: 150 },
  { label: 'Baseball Bat',sub: 'blunt swing',  mode: 6, params: [18, 255, 0], rl: 215, rr: 90,  action: 'pulse', dur: 160 },
  { label: 'Sledgehammer',sub: 'crushing',     mode: 6, params: [10, 255, 0], rl: 255, rr: 70,  action: 'pulse', dur: 200 },
  { label: 'Block',       sub: 'guard brace',  side: 0, mode: 1, params: [70, 200, 0], rl: 0, rr: 0, action: 'hold' },
];

// Minecraft per-item / block feels.
const MC_TOOL_PRESETS = [
  { label: 'Sword',     sub: 'swing kick',   mode: 6, params: [30, 200, 0], rl: 120, rr: 160, action: 'pulse', dur: 110 },
  { label: 'Axe',       sub: 'heavy swing',  mode: 6, params: [18, 240, 0], rl: 185, rr: 120, action: 'pulse', dur: 150 },
  { label: 'Pickaxe',   sub: 'stone grind',  mode: 6, params: [9,  220, 0], rl: 90,  rr: 200, action: 'hold' },
  { label: 'Shovel',    sub: 'dirt grind',   mode: 6, params: [6,  150, 0], rl: 120, rr: 90,  action: 'hold' },
  { label: 'Bow draw',  sub: 'pull tension', side: 1, mode: 1, params: [40, 210, 0], rl: 0, rr: 0, action: 'hold' },
  { label: 'Bow release',sub: 'string twang',mode: 6, params: [35, 200, 0], rl: 80,  rr: 180, action: 'pulse', dur: 90 },
  { label: 'Shield',    sub: 'L2 brace',     side: 0, mode: 1, params: [60, 220, 0], rl: 0, rr: 0, action: 'hold' },
  { label: 'Eat',       sub: 'gulp pulse',   mode: 6, params: [12, 120, 0], rl: 100, rr: 60,  action: 'double' },
];
const MC_BLOCK_PRESETS = [
  { label: 'Break block', sub: 'thud',         mode: 6, params: [16, 220, 0], rl: 200, rr: 100, action: 'pulse', dur: 130 },
  { label: 'Place block', sub: 'soft tap',     mode: 6, params: [30, 140, 0], rl: 110, rr: 80,  action: 'pulse', dur: 80  },
  { label: 'Damage',      sub: 'hurt jolt',    mode: 6, params: [20, 255, 0], rl: 255, rr: 200, action: 'pulse', dur: 150 },
  { label: 'Low health',  sub: 'heartbeat',    mode: 6, params: [10, 200, 0], rl: 220, rr: 60,  action: 'double' },
  { label: 'Sprint',      sub: 'footfalls',    mode: 6, params: [8,  130, 0], rl: 90,  rr: 70,  action: 'hold' },
];

// Audio reactive bands — bass = left motor, treble = right motor.
const AUDIO_PRESETS = [
  { label: 'Bass hit',    sub: 'left thump',   mode: 0, params: [], rl: 230, rr: 0,   action: 'pulse', dur: 140 },
  { label: 'Treble buzz', sub: 'right fizz',   mode: 0, params: [], rl: 0,   rr: 200, action: 'hold' },
  { label: 'Full mix',    sub: 'both motors',  mode: 0, params: [], rl: 200, rr: 200, action: 'hold' },
  { label: 'Kick + hat',  sub: 'beat combo',   mode: 0, params: [], rl: 240, rr: 120, action: 'burst3' },
];

buildPresetGrid('melee-presets',    MELEE_PRESETS);
buildPresetGrid('mc-tool-presets',  MC_TOOL_PRESETS);
buildPresetGrid('mc-block-presets', MC_BLOCK_PRESETS);
buildPresetGrid('audio-presets',    AUDIO_PRESETS);

// Stop buttons inside the preview tabs.
document.querySelectorAll('#lab-panel .lab-stop').forEach(btn =>
  btn.addEventListener('click', labStop));

// ─── Live preview — route the real engine to a profile, drive it with the pad ─
// Selector data per tab. Each maps a button to an invoke command + key.
const GUN_WEAPON_SEL = [
  ['pistol','Pistol'], ['revolver','Revolver'], ['rifle','Rifle'], ['burst','Burst'],
  ['ar','AR Auto'], ['smg','SMG'], ['lmg','LMG'], ['shotgun','Shotgun'], ['sniper','Sniper'],
];
const MELEE_WEAPON_SEL = [
  ['fists','Fists'], ['knife','Knife'], ['machete','Machete'], ['katana','Katana'],
  ['axe','Axe'], ['cleaver','Cleaver'], ['knuckles','Knuckles'], ['bat','Bat / Pipe'],
  ['spear','Spear'], ['sledge','Sledgehammer'],
];
const MC_ITEM_SEL = [
  ['sword','Sword'], ['axe','Axe'], ['pickaxe','Pickaxe'], ['shovel','Shovel'],
  ['hoe','Hoe'], ['bow','Bow'], ['crossbow','Crossbow'], ['trident','Trident'],
  ['shield','Shield'], ['food','Food'], ['block','Block'],
];

// Build a selector grid that highlights the active choice and fires `cmd` on click.
// `onSelect(key)` runs after the command resolves (used to open the feel editor).
function buildSelectorGrid(containerId, items, cmd, argName, onSelect) {
  const el = document.getElementById(containerId);
  if (!el) return;
  items.forEach(([key, label]) => {
    const btn = document.createElement('button');
    btn.className = 'lab-preset lab-sel-btn';
    btn.dataset.key = key;
    btn.textContent = label;
    btn.addEventListener('click', async () => {
      await invoke(cmd, { [argName]: key });
      el.querySelectorAll('.lab-sel-btn').forEach(b =>
        b.classList.toggle('selected', b === btn));
      if (onSelect) onSelect(key);
    });
    el.appendChild(btn);
  });
}

buildSelectorGrid('gun-weapon-sel',   GUN_WEAPON_SEL,   'set_gun_weapon',   'key',  k => renderFeelEditor('gun', k));
buildSelectorGrid('melee-weapon-sel', MELEE_WEAPON_SEL, 'set_melee_weapon', 'key',  k => renderFeelEditor('melee', k));
buildSelectorGrid('mc-item-sel',      MC_ITEM_SEL,      'set_mc_item',      'item');

// ─── Feel editor — tune weapon values with sliders, save to feels.json, no rebuild ─
// Tunable fields per kind: [jsonKey, label, min, max, step].
const GUN_FIELDS = [
  ['kick_freq',   'Kick freq',    0, 60,  1],
  ['kick_amp',    'Kick amp',     0, 255, 1],
  ['rumble_l',    'Rumble L',     0, 255, 1],
  ['rumble_r',    'Rumble R',     0, 255, 1],
  ['burst_count', 'Burst count',  1, 5,   1],
  ['rate_hz',     'Rate Hz',      0, 30,  1],
  ['kick_frames', 'Kick frames',  1, 12,  1],
];
const MELEE_FIELDS = [
  ['swing_force',   'Swing force',   0, 255, 1],
  ['swing_exp',     'Swing curve',   1, 3,   0.1],
  ['impact_freq',   'Impact freq',   0, 60,  1],
  ['impact_force',  'Impact force',  0, 255, 1],
  ['impact_frames', 'Impact frames', 1, 12,  1],
  ['rumble_l',      'Rumble L',      0, 255, 1],
  ['rumble_r',      'Rumble R',      0, 255, 1],
];

let feelsData = { guns: [], melee: [] };
const feelSelected = { gun: null, melee: null };

invoke('get_feels').then(f => { feelsData = f; }).catch(() => {});

function renderFeelEditor(kind, key) {
  feelSelected[kind] = key;
  const editor  = document.getElementById(kind + '-feel-editor');
  const actions = document.getElementById(kind + '-feel-actions');
  if (!editor) return;
  const list   = kind === 'gun' ? feelsData.guns : feelsData.melee;
  const fields = kind === 'gun' ? GUN_FIELDS : MELEE_FIELDS;
  const tune   = list.find(t => t.key === key);
  if (!tune) { editor.innerHTML = ''; if (actions) actions.style.display = 'none'; return; }

  editor.innerHTML = '';
  fields.forEach(([fkey, label, min, max, step]) => {
    const row = document.createElement('div');
    row.className = 'feel-field';
    const fractional = step < 1;
    const val = fractional ? Number(tune[fkey]).toFixed(1) : tune[fkey];
    row.innerHTML =
      `<label>${label}</label>` +
      `<input type="range" min="${min}" max="${max}" step="${step}" value="${tune[fkey]}">` +
      `<span class="feel-val">${val}</span>`;
    const input = row.querySelector('input');
    const out   = row.querySelector('.feel-val');
    input.addEventListener('input', () => {
      const v = fractional ? parseFloat(input.value) : parseInt(input.value, 10);
      tune[fkey] = v;
      out.textContent = fractional ? v.toFixed(1) : v;
    });
    editor.appendChild(row);
  });
  if (actions) actions.style.display = 'flex';
}

// Save / reset buttons.
document.querySelectorAll('.feel-save').forEach(btn => {
  btn.addEventListener('click', async () => {
    await invoke('save_feels', { feels: feelsData });
    const orig = btn.textContent;
    btn.textContent = '✓ Saved';
    setTimeout(() => { btn.textContent = orig; }, 1200);
  });
});
document.querySelectorAll('.feel-reset').forEach(btn => {
  btn.addEventListener('click', async () => {
    feelsData = await invoke('reset_feels');
    const kind = btn.dataset.kind;
    if (feelSelected[kind]) renderFeelEditor(kind, feelSelected[kind]);
  });
});

// Highlight the selector matching a stored key (called from snapshot on tab enter).
function syncSelector(containerId, key) {
  const el = document.getElementById(containerId);
  if (!el) return;
  el.querySelectorAll('.lab-sel-btn').forEach(b =>
    b.classList.toggle('selected', b.dataset.key === key));
}

let labPreviewProfile = null; // currently live-previewed profile, or null

async function setPreview(profile, active) {
  await invoke('set_preview', { active, profile });
  labPreviewProfile = active ? profile : null;
}

// Turn off whatever preview is running and reset every toggle's UI.
async function stopPreview() {
  if (labPreviewProfile) await setPreview(labPreviewProfile, false);
  document.querySelectorAll('.lab-live-toggle').forEach(b => {
    b.classList.remove('on');
    b.textContent = '⚡ Live preview: OFF';
  });
}

document.querySelectorAll('.lab-live-toggle').forEach(btn => {
  btn.addEventListener('click', async () => {
    const profile = btn.dataset.profile;
    const turningOn = !btn.classList.contains('on');
    await stopPreview();          // only one preview at a time
    if (turningOn) {
      await setPreview(profile, true);
      btn.classList.add('on');
      btn.textContent = '⚡ Live preview: ON';
    }
  });
});

// Enter the Trigger tab: sync its controls.
function labEnterTrigger() {
  labSideBtns.forEach(b => b.classList.toggle('active-gun', parseInt(b.dataset.side, 10) === labState.side));
  labModeSel.value = String(labState.mode);
  labRebuildParams();
}

// Switch the visible tab. Stops every tab's effects first so they don't fight.
async function showLabTab(tab) {
  labActiveTab = tab;
  labTabs.forEach(b => b.classList.toggle('active', b.dataset.tab === tab));
  labTabContents.forEach(c => c.classList.toggle('hidden', c.dataset.tab !== tab));

  // Clear any running effect from the tab we just left.
  labStop();
  rcStop();
  await stopPreview();

  if (tab === 'gun')         labEnterTrigger();
  else if (tab === 'racing') await rcEnter();
  // melee / minecraft / audio / static: preview-only, nothing to initialize.

  // Sync the live selectors to the engine's stored choices.
  if (tab === 'gun' || tab === 'melee' || tab === 'minecraft') {
    try {
      const s = await invoke('get_state');
      if (tab === 'gun')       syncSelector('gun-weapon-sel',   s.gun_weapon);
      if (tab === 'melee')     syncSelector('melee-weapon-sel', s.melee_weapon);
      if (tab === 'minecraft') syncSelector('mc-item-sel',      s.mc_item);
    } catch (_) { /* ignore */ }
  }

  requestAnimationFrame(() => setLabWindow(true, labPanel));
}

const labUpsell      = document.getElementById('lab-upsell');
const labUpsellClose = document.getElementById('lab-upsell-close');

labToggle.addEventListener('click', () => {
  // Pro gate — non-Pro users get the upsell instead of the Lab.
  if (!currentPro) {
    if (labUpsell) labUpsell.classList.remove('hidden');
    return;
  }
  const opening = labPanel.classList.contains('hidden');
  if (opening) {
    labPanel.classList.remove('hidden');
    showLabTab(labActiveTab);
  } else {
    closeLab();
  }
});

if (labUpsellClose) labUpsellClose.addEventListener('click', () => labUpsell.classList.add('hidden'));
if (labUpsell) labUpsell.addEventListener('click', e => { if (e.target === labUpsell) labUpsell.classList.add('hidden'); });

labTabs.forEach(btn => btn.addEventListener('click', () => {
  if (btn.dataset.tab !== labActiveTab) showLabTab(btn.dataset.tab);
}));

async function closeLab() {
  labStop();
  rcStop();
  await stopPreview();
  labPanel.classList.add('hidden');
  setLabWindow(false);
}

labClose.addEventListener('click', closeLab);

// ─── Motion panel — tilt steering + gyro aim ────────────────────────────────
const motionToggle = document.getElementById('motion-toggle');
const motionPanel  = document.getElementById('motion-panel');
const motionClose  = document.getElementById('motion-close');

// Local mirror of the backend MotionCfg. Pushed on every control change.
const motionCfg = {
  steer: { enabled: false, sens: 1.0, deadzone: 3, max_deg: 45, invert: false, axis: 0 },
  aim:   { enabled: false, mode: 0, sens_x: 12, sens_y: 12, deadzone: 1.5, invert_y: false },
};

const pushSteer = () => invoke('set_motion_steer', { cfg: {
  enabled:  motionCfg.steer.enabled,
  sens:     motionCfg.steer.sens,
  deadzone: motionCfg.steer.deadzone,
  max_deg:  motionCfg.steer.max_deg,
  invert:   motionCfg.steer.invert,
  axis:     motionCfg.steer.axis,
}}).catch(() => {});

const pushAim = () => invoke('set_motion_aim', { cfg: {
  enabled:  motionCfg.aim.enabled,
  mode:     motionCfg.aim.mode,
  sens_x:   motionCfg.aim.sens_x,
  sens_y:   motionCfg.aim.sens_y,
  deadzone: motionCfg.aim.deadzone,
  invert_y: motionCfg.aim.invert_y,
}}).catch(() => {});

// Steering control refs
const steerEnable = document.getElementById('steer-enable');
const steerAxisBtns = document.querySelectorAll('#steer-axis-btns .pill');
const steerSens = document.getElementById('steer-sens');
const steerDz   = document.getElementById('steer-dz');
const steerMax  = document.getElementById('steer-max');
const steerInvert = document.getElementById('steer-invert');
// Aim control refs
const aimEnable = document.getElementById('aim-enable');
const aimModeBtns = document.querySelectorAll('#aim-mode-btns .pill');
const aimSx = document.getElementById('aim-sx');
const aimSy = document.getElementById('aim-sy');
const aimDz = document.getElementById('aim-dz');
const aimInvert = document.getElementById('aim-invert');
// Viz refs
const wheelRotor = document.getElementById('wheel-rotor');
const steerTiltVal = document.getElementById('steer-tilt-val');
const aimDot = document.getElementById('aim-dot');
const aimStateVal = document.getElementById('aim-state-val');

// Reflect the whole motion UI from the local cfg.
function syncMotionUI() {
  steerEnable.classList.toggle('on', motionCfg.steer.enabled);
  steerEnable.textContent = `Tilt Steering: ${motionCfg.steer.enabled ? 'ON' : 'OFF'}`;
  steerAxisBtns.forEach(b => b.classList.toggle('m-active', +b.dataset.axis === motionCfg.steer.axis));
  steerSens.value = Math.round(motionCfg.steer.sens * 100);
  document.getElementById('steer-sens-val').textContent = motionCfg.steer.sens.toFixed(2);
  steerDz.value = motionCfg.steer.deadzone;
  document.getElementById('steer-dz-val').textContent = `${motionCfg.steer.deadzone}°`;
  steerMax.value = motionCfg.steer.max_deg;
  document.getElementById('steer-max-val').textContent = `${motionCfg.steer.max_deg}°`;
  steerInvert.classList.toggle('on', motionCfg.steer.invert);

  aimEnable.classList.toggle('on', motionCfg.aim.enabled);
  aimEnable.textContent = `Gyro Aim: ${motionCfg.aim.enabled ? 'ON' : 'OFF'}`;
  aimModeBtns.forEach(b => b.classList.toggle('m-active', +b.dataset.mode === motionCfg.aim.mode));
  aimSx.value = motionCfg.aim.sens_x;
  document.getElementById('aim-sx-val').textContent = motionCfg.aim.sens_x;
  aimSy.value = motionCfg.aim.sens_y;
  document.getElementById('aim-sy-val').textContent = motionCfg.aim.sens_y;
  aimDz.value = Math.round(motionCfg.aim.deadzone * 10);
  document.getElementById('aim-dz-val').textContent = motionCfg.aim.deadzone.toFixed(1);
  aimInvert.classList.toggle('on', motionCfg.aim.invert_y);
}

// Load current values from the backend snapshot (so the panel opens in sync).
function initMotionFromState(s) {
  motionCfg.steer.enabled  = !!s.steer_enabled;
  motionCfg.steer.sens     = s.steer_sens ?? 1.0;
  motionCfg.steer.deadzone = s.steer_deadzone ?? 3;
  motionCfg.steer.max_deg  = s.steer_max_deg ?? 45;
  motionCfg.steer.invert   = !!s.steer_invert;
  motionCfg.steer.axis     = s.steer_axis ?? 0;
  motionCfg.aim.enabled    = !!s.aim_enabled;
  motionCfg.aim.mode       = s.aim_mode ?? 0;
  motionCfg.aim.sens_x     = s.aim_sens_x ?? 12;
  motionCfg.aim.sens_y     = s.aim_sens_y ?? 12;
  motionCfg.aim.deadzone   = s.aim_deadzone ?? 1.5;
  motionCfg.aim.invert_y   = !!s.aim_invert_y;
  syncMotionUI();
}

// Animate the wheel + reticle from the live snapshot.
function updateMotionViz(s) {
  const tilt = s.motion_tilt || 0;
  const shown = motionCfg.steer.invert ? -tilt : tilt;
  if (wheelRotor) wheelRotor.setAttribute('transform', `rotate(${(shown * 2).toFixed(1)} 60 60)`);
  if (steerTiltVal) steerTiltVal.textContent = `${tilt.toFixed(0)}°`;
  // Aim reticle: map raw gyro rate to a clamped offset around center (60,60).
  const clamp = (v, m) => Math.max(-m, Math.min(m, v));
  const yaw = clamp((s.gyro_yaw || 0) / 120, 38);
  const pitch = clamp((s.gyro_pitch || 0) / 120, 38) * (motionCfg.aim.invert_y ? -1 : 1);
  if (aimDot) {
    aimDot.setAttribute('cx', (60 + yaw).toFixed(1));
    aimDot.setAttribute('cy', (60 + pitch).toFixed(1));
    const moving = Math.abs(s.gyro_yaw || 0) + Math.abs(s.gyro_pitch || 0) > 200;
    aimDot.classList.toggle('live', moving && motionCfg.aim.enabled);
  }
  if (aimStateVal) {
    aimStateVal.textContent = !motionCfg.aim.enabled ? 'off'
      : motionCfg.aim.mode === 0 ? 'on'
      : (s.aim_active || (motionCfg.aim.mode === 1 && s.touchpad_btn)) ? 'aiming' : 'ready';
  }
}

// ── Control wiring ──
steerEnable.addEventListener('click', () => { motionCfg.steer.enabled = !motionCfg.steer.enabled; syncMotionUI(); pushSteer(); });
steerInvert.addEventListener('click', () => { motionCfg.steer.invert = !motionCfg.steer.invert; syncMotionUI(); pushSteer(); });
steerAxisBtns.forEach(b => b.addEventListener('click', () => { motionCfg.steer.axis = +b.dataset.axis; syncMotionUI(); pushSteer(); }));
steerSens.addEventListener('input', () => { motionCfg.steer.sens = +steerSens.value / 100; syncMotionUI(); pushSteer(); });
steerDz.addEventListener('input', () => { motionCfg.steer.deadzone = +steerDz.value; syncMotionUI(); pushSteer(); });
steerMax.addEventListener('input', () => { motionCfg.steer.max_deg = +steerMax.value; syncMotionUI(); pushSteer(); });

aimEnable.addEventListener('click', () => { motionCfg.aim.enabled = !motionCfg.aim.enabled; syncMotionUI(); pushAim(); });
aimInvert.addEventListener('click', () => { motionCfg.aim.invert_y = !motionCfg.aim.invert_y; syncMotionUI(); pushAim(); });
aimModeBtns.forEach(b => b.addEventListener('click', () => { motionCfg.aim.mode = +b.dataset.mode; syncMotionUI(); pushAim(); }));
aimSx.addEventListener('input', () => { motionCfg.aim.sens_x = +aimSx.value; syncMotionUI(); pushAim(); });
aimSy.addEventListener('input', () => { motionCfg.aim.sens_y = +aimSy.value; syncMotionUI(); pushAim(); });
aimDz.addEventListener('input', () => { motionCfg.aim.deadzone = +aimDz.value / 10; syncMotionUI(); pushAim(); });

async function closeMotion() {
  motionPanel.classList.add('hidden');
  motionToggle.classList.remove('on');
  setLabWindow(false);
}

motionToggle.addEventListener('click', async () => {
  const opening = motionPanel.classList.contains('hidden');
  if (opening) {
    if (!labPanel.classList.contains('hidden')) closeLab();
    const s = await invoke('get_state');
    initMotionFromState(s);
    motionPanel.classList.remove('hidden');
    motionToggle.classList.add('on');
    requestAnimationFrame(() => setLabWindow(true, motionPanel));
  } else {
    closeMotion();
  }
});
motionClose.addEventListener('click', closeMotion);

// ─── Racing Lab tab — personalize brake / throttle feel ─────────────────────
const rcPreviewBtn    = document.getElementById('rc-preview');
const rcSaveBtn       = document.getElementById('rc-save');
const rcClearBtn      = document.getElementById('rc-clear');
const rcStatus        = document.getElementById('rc-status');
const rcTireScrubBtn  = document.getElementById('rc-tire-scrub');
const rcThrottleLight = document.getElementById('rc-throttle-light');

// Each slider: [element id, curve key, isExp (÷10 display)]
const RC_SLIDERS = [
  ['rc-brake-start',    'brake_start',    false],
  ['rc-brake-end',      'brake_end',      false],
  ['rc-brake-exp',      'brake_exp',      true ],
  ['rc-feather-end',    'feather_end',    false],
  ['rc-throttle-start', 'throttle_start', false],
  ['rc-throttle-end',   'throttle_end',   false],
  ['rc-throttle-exp',   'throttle_exp',   true ],
  ['rc-engine-texture', 'engine_texture', false],
  ['rc-abs-freq',       'abs_freq',       false],
  ['rc-abs-delay',      'abs_delay',      false],
  ['rc-shift-force',    'shift_force',    false],
];

// STRENGTHS table mirror (must match hid.rs) — for the preset buttons.
const RC_PRESETS = {
  light:  { brake_start: 120, brake_end: 215, brake_exp: 1.5, throttle_start: 30, throttle_end: 108, throttle_exp: 1.3, shift_force: 230 },
  medium: { brake_start: 140, brake_end: 238, brake_exp: 1.7, throttle_start: 45, throttle_end: 140, throttle_exp: 1.4, shift_force: 245 },
  hard:   { brake_start: 158, brake_end: 255, brake_exp: 1.9, throttle_start: 58, throttle_end: 172, throttle_exp: 1.4, shift_force: 255 },
  max:    { brake_start: 175, brake_end: 255, brake_exp: 2.3, throttle_start: 75, throttle_end: 200, throttle_exp: 1.6, shift_force: 255 },
};

const rcState = { preview: false };

function rcReadCurve() {
  const c = {};
  RC_SLIDERS.forEach(([id, key, isExp]) => {
    const raw = parseInt(document.getElementById(id).value, 10);
    c[key] = isExp ? raw / 10 : raw;
  });
  return c;
}

// Sets only the keys present in `c` — preset buttons pass just the 7 strength
// fields and leave the ABS/engine/bite knobs untouched.
function rcSetSliders(c) {
  RC_SLIDERS.forEach(([id, key, isExp]) => {
    if (c[key] === undefined || c[key] === null) return;
    const slider = document.getElementById(id);
    const val    = document.getElementById(id + '-val');
    const raw    = isExp ? Math.round(c[key] * 10) : c[key];
    slider.value = String(raw);
    val.textContent = isExp ? (raw / 10).toFixed(1) : String(raw);
  });
}

function rcSend(active) {
  invoke('set_racing_lab', { active, curve: rcReadCurve() });
}

// Wire each slider: update its label, and push live if preview is on.
RC_SLIDERS.forEach(([id, , isExp]) => {
  const slider = document.getElementById(id);
  const val    = document.getElementById(id + '-val');
  slider.addEventListener('input', () => {
    const raw = parseInt(slider.value, 10);
    val.textContent = isExp ? (raw / 10).toFixed(1) : String(raw);
    if (rcState.preview) rcSend(true);
  });
});

document.querySelectorAll('#racing-presets .lab-preset').forEach(btn => {
  btn.addEventListener('click', () => {
    rcSetSliders(RC_PRESETS[btn.dataset.rcPreset]);
    if (rcState.preview) rcSend(true);
    rcStatus.textContent = `Loaded ${btn.dataset.rcPreset} preset into sliders.`;
  });
});

// ── Drivetrain feel — applies live to the Racing profile, independent of the
//    custom curve preview/save (it tunes the base engine character directly). ──
const dtWeight = document.getElementById('rc-dt-weight');
const dtTakeup = document.getElementById('rc-dt-takeup');
const dtIdle   = document.getElementById('rc-dt-idle');
const dtRed    = document.getElementById('rc-dt-red');
const dtLoad   = document.getElementById('rc-dt-load');
const dtTachRpm  = document.getElementById('rc-tach-rpm');
const dtTachLoad = document.getElementById('rc-tach-load');
const dtCfg = { weight: 40, take_up: 42, idle_hz: 7, red_hz: 26, load: 50 };

const pushDrivetrain = () => invoke('set_drivetrain', { cfg: { ...dtCfg } }).catch(() => {});

function syncDrivetrainUI() {
  dtWeight.value = dtCfg.weight;  document.getElementById('rc-dt-weight-val').textContent = dtCfg.weight;
  dtTakeup.value = dtCfg.take_up; document.getElementById('rc-dt-takeup-val').textContent = dtCfg.take_up;
  dtIdle.value   = dtCfg.idle_hz; document.getElementById('rc-dt-idle-val').textContent = `${dtCfg.idle_hz} Hz`;
  dtRed.value    = dtCfg.red_hz;  document.getElementById('rc-dt-red-val').textContent = `${dtCfg.red_hz} Hz`;
  dtLoad.value   = dtCfg.load;    document.getElementById('rc-dt-load-val').textContent = dtCfg.load;
}

function initDrivetrainFromState(s) {
  dtCfg.weight  = s.dt_weight ?? 40;
  dtCfg.take_up = s.dt_take_up ?? 42;
  dtCfg.idle_hz = s.dt_idle_hz ?? 7;
  dtCfg.red_hz  = s.dt_red_hz ?? 26;
  dtCfg.load    = s.dt_load ?? 50;
  syncDrivetrainUI();
}

dtWeight.addEventListener('input', () => { dtCfg.weight  = +dtWeight.value; syncDrivetrainUI(); pushDrivetrain(); });
dtTakeup.addEventListener('input', () => { dtCfg.take_up = +dtTakeup.value; syncDrivetrainUI(); pushDrivetrain(); });
dtIdle.addEventListener('input',   () => { dtCfg.idle_hz = +dtIdle.value;   syncDrivetrainUI(); pushDrivetrain(); });
dtRed.addEventListener('input',    () => { dtCfg.red_hz  = +dtRed.value;    syncDrivetrainUI(); pushDrivetrain(); });
dtLoad.addEventListener('input',   () => { dtCfg.load    = +dtLoad.value;   syncDrivetrainUI(); pushDrivetrain(); });

// Live tach — only update while the Racing Lab tab is visible (cheap guard).
function updateTach(s) {
  if (!dtTachRpm) return;
  dtTachRpm.style.width  = `${Math.round((s.eng_rpm || 0) * 100)}%`;
  dtTachLoad.style.width = `${Math.round(Math.min(1, (s.eng_load || 0)) * 100)}%`;
}

// ── Drivetrain profile ─────────────────────────────────────────────────────
const dtProfile  = document.getElementById('rc-dt-profile');
const dtAutoBtn  = document.getElementById('rc-dt-auto');
let   dtAutoState = false;

function initDtProfileFromState(s) {
  if (dtProfile && s.dt_profile !== undefined) {
    dtProfile.value = s.dt_profile;
  }
  dtAutoState = !!s.dt_auto;
  if (dtAutoBtn) dtAutoBtn.classList.toggle('on', dtAutoState);
}

dtProfile?.addEventListener('change', () => {
  invoke('set_drivetrain_profile', { idx: +dtProfile.value }).catch(() => {});
});

dtAutoBtn?.addEventListener('click', () => {
  dtAutoState = !dtAutoState;
  dtAutoBtn.classList.toggle('on', dtAutoState);
  invoke('set_drivetrain_auto', { enabled: dtAutoState }).catch(() => {});
});

// ── Game source ─────────────────────────────────────────────────────────
const gameSourceSel = document.getElementById('rc-game-source');
gameSourceSel?.addEventListener('change', () => {
  invoke('set_game_source', { source: gameSourceSel.value }).catch(() => {});
});

rcPreviewBtn.addEventListener('click', () => {
  rcState.preview = !rcState.preview;
  rcPreviewBtn.classList.toggle('on', rcState.preview);
  rcSend(rcState.preview);
  rcStatus.textContent = rcState.preview
    ? 'Live preview ON — pull L2 / R2 to feel the curve.'
    : 'Live preview off.';
});

rcSaveBtn.addEventListener('click', async () => {
  const on = await invoke('save_racing_custom', { enabled: true, curve: rcReadCurve() });
  rcStatus.textContent = on
    ? 'Saved. This is now your Racing profile (overrides Light/Medium/Hard/Max).'
    : 'Saved.';
  soundOn();
});

rcClearBtn.addEventListener('click', async () => {
  await invoke('save_racing_custom', { enabled: false, curve: rcReadCurve() });
  rcStatus.textContent = 'Reverted — Racing now uses the strength presets.';
});

// Steering FX toggles — independent of the curve, persisted on the Rust side.
const rcFx = { tireScrub: false, throttleLight: false };

function rcSendFx() {
  invoke('set_steering_fx', { tireScrub: rcFx.tireScrub, throttleLight: rcFx.throttleLight });
}

rcTireScrubBtn.addEventListener('click', () => {
  rcFx.tireScrub = !rcFx.tireScrub;
  rcTireScrubBtn.classList.toggle('on', rcFx.tireScrub);
  rcSendFx();
});

rcThrottleLight.addEventListener('click', () => {
  rcFx.throttleLight = !rcFx.throttleLight;
  rcThrottleLight.classList.toggle('on', rcFx.throttleLight);
  rcSendFx();
});

function rcStop() {
  if (rcState.preview) {
    rcState.preview = false;
    rcPreviewBtn.classList.remove('on');
    rcSend(false);
  }
}

// ─── Lab window sizing ──────────────────────────────────────────────────────
// Labs are full-window overlays, so widen the native window while one is open
// and grow its height to fit the content. Restores to the view-mode size on close.
async function setLabWindow(open, panel) {
  try {
    if (open) {
      // Measure the panel's natural height after layout, clamp to a sane range.
      const h = Math.min(Math.max(Math.ceil(panel.scrollHeight) + 24, 420), 900);
      await invoke('set_window_size', { width: 1000, height: h });
    } else {
      const [w, h] = viewMode === 'compact' ? [560, 330] : [820, 540];
      await invoke('set_window_size', { width: w, height: h });
    }
  } catch (_) { /* sizing is best-effort; never block the UI */ }
}

// Entering the Racing tab: initialize sliders + FX toggles from saved state.
async function rcEnter() {
  const s = await invoke('get_state');
  rcSetSliders({
    brake_start:    s.rc_brake_start,
    brake_end:      s.rc_brake_end,
    brake_exp:      s.rc_brake_exp,
    feather_end:    s.rc_feather_end,
    throttle_start: s.rc_throttle_start,
    throttle_end:   s.rc_throttle_end,
    throttle_exp:   s.rc_throttle_exp,
    engine_texture: s.rc_engine_texture,
    abs_freq:       s.rc_abs_freq,
    abs_delay:      s.rc_abs_delay,
    shift_force:    s.rc_shift_force,
  });
  rcFx.tireScrub     = !!s.tire_scrub_on;
  rcFx.throttleLight = !!s.throttle_light_on;
  rcTireScrubBtn.classList.toggle('on', rcFx.tireScrub);
  rcThrottleLight.classList.toggle('on', rcFx.throttleLight);
  rcStatus.textContent = s.racing_custom_on
    ? 'Custom curve is active as your Racing profile.'
    : 'Using strength presets. Tune + Save to personalize.';
  initDrivetrainFromState(s);
}
