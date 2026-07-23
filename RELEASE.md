# Release & Beta Distribution

How to ship a build to Gumroad and hand beta testers free keys.

---

## One-time worker setup

The license worker lives in `worker/`. It verifies Gumroad purchases, binds
machines, and now also mints free **beta keys** that skip Gumroad entirely.

Set the three secrets (only needed once, or when rotating):

```bash
cd worker
npx wrangler secret put GUMROAD_PRODUCT_ID   # from your Gumroad product page
npx wrangler secret put TOKEN_SECRET         # any long random string
npx wrangler secret put ADMIN_KEY            # any long random string — gates /admin/*
```

Generate strong random values for `TOKEN_SECRET` and `ADMIN_KEY`:

```bash
openssl rand -hex 32
```

Deploy the worker:

```bash
cd worker
npm install        # first time only
npm run typecheck  # optional sanity check
npm run deploy
```

---

## Build the app (universal DMG)

Universal = runs on both Apple Silicon and Intel Macs. Build both targets first
time:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run build -- --target universal-apple-darwin
```

Output:

```
src-tauri/target/universal-apple-darwin/release/bundle/dmg/Universal DualSense Haptics_<version>_universal.dmg
```

That `.dmg` is the file you upload to Gumroad.

> Single-arch (faster, this machine only): `npm run build` — output under
> `src-tauri/target/release/bundle/dmg/`.

### Gatekeeper note (unsigned build)

`signingIdentity` is `null`, so the DMG is unsigned. Testers will see
"app is damaged / from an unidentified developer." Give them this one-liner to
clear the quarantine flag after dragging the app to Applications:

```bash
xattr -dr com.apple.quarantine "/Applications/Universal DualSense Haptics.app"
```

(Signing + notarization is a future step — see `ROADMAP.md`.)

---

## Upload to Gumroad

1. Gumroad product → **Content** → upload the universal `.dmg`.
2. Make sure **"Generate a unique license key per sale"** is ON (Settings →
   product → "License keys"). The app reads that key.
3. Set the price (or "pay what you want" with a $0 minimum for an open beta).
4. Publish. The download URL the app points at is
   `https://alt3red.gumroad.com/l/universal-dualsense-haptics` — update it in
   `worker/src/index.ts` (`handleVersion`) if the slug changes, then redeploy.

After publishing a new version, bump the in-app "latest" pointer so older
clients see the update:

```bash
cd worker
ADMIN_KEY=... VER=0.3.0 \
  curl -s -X POST "$LICENSE_SERVER/admin/set-latest-version" \
  -H "X-Admin-Key: $ADMIN_KEY" -H 'Content-Type: application/json' \
  -d "{\"version\":\"$VER\"}"
```

---

## Beta testers — free keys, no purchase

Beta keys activate the app without anyone paying or going through Gumroad
checkout. They look like `BETA-XXXX-XXXX-XXXX` and run through the exact same
`/activate` flow, so the app needs zero changes.

Set your admin key once per shell:

```bash
export ADMIN_KEY="<the value you set with wrangler secret put ADMIN_KEY>"
```

Mint keys:

```bash
cd worker
./scripts/beta.sh mint 10 "discord wave 1"   # 10 keys, labelled
# → {"ok":true,"count":10,"keys":["BETA-...","BETA-...", ...]}
```

Hand each tester one key. They open the app, paste it into the license prompt,
and they're on Full Immersion. Each key allows 2 machines by default.

Manage keys:

```bash
./scripts/beta.sh list                       # all minted beta keys + machine counts
./scripts/beta.sh revoke BETA-AB12-CD34-EF56 # disable a key (refuses activate + validate)
./scripts/beta.sh unrevoke BETA-AB12-CD34-EF56
```

> `mint` options: `mint [count] [note]`. Count caps at 100 per call, machines
> per key default 2 (override only via the raw endpoint).

---

## Kill switch / forcing upgrades

To brick clients older than a version (e.g. after a critical fix):

```bash
cd worker
ADMIN_KEY=... VER=0.3.0 npm run brick
```

Anything below `VER` gets "Update required" on next launch/validate.

---

## Admin endpoint reference

All `/admin/*` routes require header `X-Admin-Key: <ADMIN_KEY>`.

| Method | Path | Body | Purpose |
|--------|------|------|---------|
| POST | `/activate` | `{key, machineId, appVersion}` | client activation (Gumroad or beta) |
| POST | `/validate` | `{machineId, token, appVersion}` | client revalidation |
| GET  | `/version` | — | latest version + download URL |
| POST | `/admin/mint-beta` | `{count?, maxMachines?, note?}` | generate beta keys |
| POST | `/admin/revoke` | `{key, revoked?}` | disable/enable a key |
| GET  | `/admin/list-beta` | — | list beta keys |
| POST | `/admin/set-min-version` | `{version}` | brick older clients |
| POST | `/admin/set-latest-version` | `{version}` | update in-app latest pointer |
