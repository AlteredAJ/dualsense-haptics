/**
 * Universal DualSense Haptics — License Worker  v1.3
 *
 * Routes:
 *   GET  /version               — latest app version + download URL
 *   POST /activate              — validate a Gumroad key, bind machine, issue token
 *   POST /validate              — check an existing token; enforces minimum app version
 *   POST /admin/set-min-version — remotely brick older clients
 *   POST /admin/set-latest-version — update the latest version shown in-app
 *   POST /admin/mint-beta       — generate comp'd beta keys (skip Gumroad)
 *   POST /admin/revoke          — disable a key (beta or paid)
 *   GET  /admin/list-beta       — list minted beta keys + activation counts
 *
 * KV schema (binding: LICENSES)
 *   license:{key}           → LicenseRecord JSON
 *   token:{token}           → key string (reverse lookup)
 *   beta:{key}              → "1" (index of minted beta keys)
 *   config:min_version      → "0.1.0"
 *   config:latest_version   → "0.3.0"
 *
 * Secrets (wrangler secret put <NAME>)
 *   GUMROAD_PRODUCT_ID  ADMIN_KEY  TOKEN_SECRET
 */

export interface Env {
  LICENSES:           KVNamespace;
  GUMROAD_PRODUCT_ID: string;
  TOKEN_SECRET:       string;
  ADMIN_KEY:          string;
}

interface MachineEntry {
  machineId:   string;
  token:       string;
  activatedAt: number;
  lastSeen:    number;
}

interface LicenseRecord {
  key:         string;
  maxMachines: number;   // 2 for all current licenses; tighten to 1 post-beta
  machines:    MachineEntry[];
  uses:        number;
  beta?:       boolean;  // true = comp'd beta key, skips Gumroad verification
  pro?:        boolean;  // true = $4 Pro tier (unlocks the Lab); false/undef = $1 Base
  note?:       string;   // optional label, e.g. tester name
  revoked?:    boolean;  // true = key disabled (activation + validation refused)
}

// If a paid key has no variant string (legacy $2.50 buyers, pre-tier), treat it as:
//   true  → grandfather them into Pro (they keep the Lab)
//   false → Base (haptics only, Lab locked)
const GRANDFATHER_EMPTY_VARIANT_AS_PRO = false;

// ─── Semver ────────────────────────────────────────────────────────────────────

function semverLt(a: string, b: string): boolean {
  const pa = a.split('.').map(Number);
  const pb = b.split('.').map(Number);
  for (let i = 0; i < 3; i++) {
    if ((pa[i] ?? 0) < (pb[i] ?? 0)) return true;
    if ((pa[i] ?? 0) > (pb[i] ?? 0)) return false;
  }
  return false;
}

// ─── Token (HMAC-SHA256 truncated) ────────────────────────────────────────────

async function makeToken(key: string, machineId: string, secret: string): Promise<string> {
  const enc = new TextEncoder();
  const cryptoKey = await crypto.subtle.importKey(
    'raw', enc.encode(secret), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']
  );
  const sig = await crypto.subtle.sign('HMAC', cryptoKey, enc.encode(`${key}|${machineId}`));
  return Array.from(new Uint8Array(sig)).map(b => b.toString(16).padStart(2, '0')).join('').slice(0, 48);
}

// ─── Gumroad ──────────────────────────────────────────────────────────────────

async function verifyGumroad(licenseKey: string, productId: string): Promise<{ ok: boolean; uses: number; pro: boolean; error?: string }> {
  const res = await fetch('https://api.gumroad.com/v2/licenses/verify', {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({ product_id: productId, license_key: licenseKey }),
  });
  const json: any = await res.json();
  if (!json.success) return { ok: false, uses: 0, pro: false, error: json.message ?? 'Invalid key' };
  // Gumroad puts the selected version/tier in purchase.variants, a string like "(Pro)".
  const variant = (json.purchase?.variants ?? '').toString();
  const pro = variant.trim() === ''
    ? GRANDFATHER_EMPTY_VARIANT_AS_PRO
    : /pro/i.test(variant);
  return { ok: true, uses: json.purchase?.uses ?? 0, pro };
}

// ─── CORS ─────────────────────────────────────────────────────────────────────

function cors(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json', 'Access-Control-Allow-Origin': '*' },
  });
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

async function handleActivate(req: Request, env: Env): Promise<Response> {
  const { key, machineId, appVersion } = await req.json<{
    key: string; machineId: string; appVersion: string;
  }>();

  if (!key || !machineId) return cors({ activated: false, error: 'Missing fields' }, 400);

  const minVer = await env.LICENSES.get('config:min_version') ?? '0.1.0';
  if (semverLt(appVersion ?? '0.0.0', minVer)) {
    return cors({ activated: false, error: `Update required — download v${minVer} at alt3red.gumroad.com` });
  }

  // Load any existing record up front — beta keys are pre-created at mint time.
  const existing = await env.LICENSES.get<LicenseRecord>(`license:${key}`, 'json');

  if (existing?.revoked) {
    return cors({ activated: false, error: 'This key has been revoked. Contact support.' });
  }

  // Beta keys skip Gumroad entirely; everything else must verify a real purchase.
  // Beta keys always grant Pro (testers need the full app, including the Lab).
  let uses = existing?.uses ?? 0;
  let pro = existing?.pro ?? false;
  if (existing?.beta) {
    pro = true;
  } else {
    const gumroad = await verifyGumroad(key, env.GUMROAD_PRODUCT_ID);
    if (!gumroad.ok) return cors({ activated: false, error: gumroad.error ?? 'Key not found' });
    uses = gumroad.uses;
    pro = gumroad.pro;
  }

  // Load or create record
  const record: LicenseRecord = existing ?? {
    key, maxMachines: 2, machines: [], uses,
  };
  record.pro = pro; // refresh tier on every activate (handles Base→Pro upgrades)

  // Already registered on this machine — return existing token (idempotent)
  const existingMachine = record.machines.find(m => m.machineId === machineId);
  if (existingMachine) {
    existingMachine.lastSeen = Date.now();
    await env.LICENSES.put(`license:${key}`, JSON.stringify(record));
    return cors({ activated: true, token: existingMachine.token, pro: record.pro });
  }

  // New machine — check capacity
  if (record.machines.length >= record.maxMachines) {
    return cors({
      activated: false,
      error: `Key already activated on ${record.maxMachines} machines. Contact support to transfer.`,
    });
  }

  // Register new machine
  const token = await makeToken(key, machineId, env.TOKEN_SECRET);
  record.machines.push({ machineId, token, activatedAt: Date.now(), lastSeen: Date.now() });
  record.uses = uses;

  await env.LICENSES.put(`license:${key}`, JSON.stringify(record));
  await env.LICENSES.put(`token:${token}`, key);

  return cors({ activated: true, token, pro: record.pro });
}

async function handleValidate(req: Request, env: Env): Promise<Response> {
  const { machineId, token, appVersion } = await req.json<{
    machineId: string; token: string; appVersion: string;
  }>();

  if (!machineId || !token) return cors({ valid: false, error: 'Missing fields' }, 400);

  const minVer = await env.LICENSES.get('config:min_version') ?? '0.1.0';
  if (semverLt(appVersion ?? '0.0.0', minVer)) {
    return cors({ valid: false, error: `Update required — download v${minVer} at alt3red.gumroad.com` });
  }

  const key = await env.LICENSES.get(`token:${token}`);
  if (!key) return cors({ valid: false });

  const record = await env.LICENSES.get<LicenseRecord>(`license:${key}`, 'json');
  if (!record) return cors({ valid: false });
  if (record.revoked) return cors({ valid: false, error: 'License revoked.' });

  const machine = record.machines.find(m => m.machineId === machineId && m.token === token);
  if (!machine) return cors({ valid: false });

  machine.lastSeen = Date.now();
  await env.LICENSES.put(`license:${key}`, JSON.stringify(record));

  return cors({ valid: true, pro: record.pro ?? false });
}

async function handleVersion(env: Env): Promise<Response> {
  const latest = await env.LICENSES.get('config:latest_version') ?? '0.3.0';
  return cors({ latest, download: 'https://alt3red.gumroad.com/l/universal-dualsense-haptics' });
}

async function handleSetMinVersion(req: Request, env: Env): Promise<Response> {
  if (req.headers.get('X-Admin-Key') !== env.ADMIN_KEY) {
    return cors({ ok: false, error: 'Unauthorized' }, 401);
  }
  const { version } = await req.json<{ version: string }>();
  if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
    return cors({ ok: false, error: 'Invalid semver' }, 400);
  }
  await env.LICENSES.put('config:min_version', version);
  return cors({ ok: true, minVersion: version });
}

async function handleSetLatestVersion(req: Request, env: Env): Promise<Response> {
  if (req.headers.get('X-Admin-Key') !== env.ADMIN_KEY) {
    return cors({ ok: false, error: 'Unauthorized' }, 401);
  }
  const { version } = await req.json<{ version: string }>();
  if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
    return cors({ ok: false, error: 'Invalid semver' }, 400);
  }
  await env.LICENSES.put('config:latest_version', version);
  return cors({ ok: true, latestVersion: version });
}

// ─── Beta keys ──────────────────────────────────────────────────────────────────

// Crockford-ish base32 (no I/O/0/1 to avoid ambiguity) for readable keys.
const KEY_ALPHABET = '23456789ABCDEFGHJKMNPQRSTUVWXYZ';

function randomBetaKey(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(12));
  const chars = Array.from(bytes, b => KEY_ALPHABET[b % KEY_ALPHABET.length]);
  // BETA-XXXX-XXXX-XXXX — the BETA- prefix makes comp'd keys obvious at a glance.
  return `BETA-${chars.slice(0, 4).join('')}-${chars.slice(4, 8).join('')}-${chars.slice(8, 12).join('')}`;
}

// POST /admin/mint-beta  { count?, maxMachines?, note? } → { ok, keys: [...] }
// Pre-creates beta license records that activate without a Gumroad purchase.
async function handleMintBeta(req: Request, env: Env): Promise<Response> {
  if (req.headers.get('X-Admin-Key') !== env.ADMIN_KEY) {
    return cors({ ok: false, error: 'Unauthorized' }, 401);
  }
  type MintBody = { count?: number; maxMachines?: number; note?: string };
  const body = await req.json<MintBody>().catch((): MintBody => ({}));
  const count = Math.min(Math.max(body.count ?? 1, 1), 100);
  const maxMachines = Math.min(Math.max(body.maxMachines ?? 2, 1), 10);

  const keys: string[] = [];
  for (let i = 0; i < count; i++) {
    let key = randomBetaKey();
    // Avoid the astronomically unlikely collision with an existing record.
    while (await env.LICENSES.get(`license:${key}`)) key = randomBetaKey();
    const record: LicenseRecord = {
      key, maxMachines, machines: [], uses: 0, beta: true, pro: true, note: body.note,
    };
    await env.LICENSES.put(`license:${key}`, JSON.stringify(record));
    await env.LICENSES.put(`beta:${key}`, '1'); // index for listing
    keys.push(key);
  }
  return cors({ ok: true, count: keys.length, keys });
}

// POST /admin/revoke  { key, revoked? } → { ok }
async function handleRevoke(req: Request, env: Env): Promise<Response> {
  if (req.headers.get('X-Admin-Key') !== env.ADMIN_KEY) {
    return cors({ ok: false, error: 'Unauthorized' }, 401);
  }
  const { key, revoked } = await req.json<{ key: string; revoked?: boolean }>();
  if (!key) return cors({ ok: false, error: 'Missing key' }, 400);
  const record = await env.LICENSES.get<LicenseRecord>(`license:${key}`, 'json');
  if (!record) return cors({ ok: false, error: 'Key not found' }, 404);
  record.revoked = revoked ?? true;
  await env.LICENSES.put(`license:${key}`, JSON.stringify(record));
  return cors({ ok: true, key, revoked: record.revoked });
}

// GET /admin/list-beta — all minted beta keys with activation counts.
async function handleListBeta(req: Request, env: Env): Promise<Response> {
  if (req.headers.get('X-Admin-Key') !== env.ADMIN_KEY) {
    return cors({ ok: false, error: 'Unauthorized' }, 401);
  }
  const list = await env.LICENSES.list({ prefix: 'beta:' });
  const keys = list.keys.map(k => k.name.slice('beta:'.length));
  const out = [];
  for (const key of keys) {
    const record = await env.LICENSES.get<LicenseRecord>(`license:${key}`, 'json');
    out.push({
      key,
      note: record?.note ?? null,
      machines: record?.machines.length ?? 0,
      maxMachines: record?.maxMachines ?? 0,
      revoked: record?.revoked ?? false,
    });
  }
  return cors({ ok: true, count: out.length, betas: out });
}

// ─── Entry ────────────────────────────────────────────────────────────────────

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    if (req.method === 'OPTIONS') {
      return new Response(null, {
        headers: { 'Access-Control-Allow-Origin': '*', 'Access-Control-Allow-Headers': '*' },
      });
    }
    const url = new URL(req.url);
    if (req.method === 'GET') {
      if (url.pathname === '/version')          return handleVersion(env);
      if (url.pathname === '/admin/list-beta')  return handleListBeta(req, env);
    }
    if (req.method === 'POST') {
      if (url.pathname === '/activate')                  return handleActivate(req, env);
      if (url.pathname === '/validate')                  return handleValidate(req, env);
      if (url.pathname === '/admin/set-min-version')     return handleSetMinVersion(req, env);
      if (url.pathname === '/admin/set-latest-version')  return handleSetLatestVersion(req, env);
      if (url.pathname === '/admin/mint-beta')           return handleMintBeta(req, env);
      if (url.pathname === '/admin/revoke')              return handleRevoke(req, env);
    }
    return cors({ error: 'Not found' }, 404);
  },
};
