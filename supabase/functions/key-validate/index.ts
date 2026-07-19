// Socrate KeyAuth validation Edge Function.
//
// WHY THIS EXISTS — the app used to talk to keyauth.win directly, with the
// KeyAuth *secret* baked into the binary. That made the license trivially
// bypassable: extract the secret, point the app at a fake KeyAuth server (a
// simple binary patch of the URL string), and have it answer "success:true"
// with a correctly-forged HMAC. See the write-up that motivated this change.
//
// Now the whole KeyAuth exchange happens HERE, server-side:
//   • The KeyAuth secret lives only in this function's env (never shipped).
//   • The verdict we return is signed with an Ed25519 PRIVATE key that also
//     never leaves the server. The app embeds only the PUBLIC key.
//
// A patcher can still redirect the app's URL to a fake server, but that fake
// server cannot produce a valid Ed25519 signature without the private key, so
// the app rejects it. The only remaining client-side attack is NOP-ing the
// signature check in the binary — which no server change can prevent, but it
// now breaks on every rebuild and leaves the attacker with no forgeable path.
//
// Protocol (POST JSON, HMAC-signed exactly like the `access` function):
//   { key, hwid, nonce } -> { valid, expiry, hwid, nonce, iat, sig }
// where `sig` = hex(Ed25519(priv, "{nonce}|{valid}|{expiry}|{hwid}|{iat}")).

import * as ed from "jsr:@noble/ed25519@2";
import { x25519 } from "jsr:@noble/curves@1/ed25519";
import { createClient } from "jsr:@supabase/supabase-js@2";

const KEYAUTH_URL = "https://keyauth.win/api/1.2/";
const KEYAUTH_VER = Deno.env.get("KEYAUTH_VER") ?? "1.0";
const KEYAUTH_NAME = Deno.env.get("KEYAUTH_APP_NAME") ?? "";
const KEYAUTH_OWNER = Deno.env.get("KEYAUTH_OWNER_ID") ?? "";
const KEYAUTH_SECRET = Deno.env.get("KEYAUTH_APP_SECRET") ?? "";

// Shared HMAC secret for inbound request auth — same scheme/secret as `access`.
const HMAC_SECRET = Deno.env.get("SOCRATE_HMAC_SECRET") ?? "";
// Ed25519 private seed (32-byte hex) used to sign the verdict. Set with:
//   supabase secrets set SOCRATE_ED25519_PRIV=<64-hex-char seed>
const ED_PRIV = Deno.env.get("SOCRATE_ED25519_PRIV") ?? "";

const MAX_SKEW = 120; // seconds

const SB_URL = Deno.env.get("SUPABASE_URL")!;
const SB_SERVICE = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!;

// ─── Resolve-chain offsets (delivered to the app, not compiled into it) ──────
// Only the two offsets that move on a Roblox update are server-controlled: the
// FakeDataModel base offset and the FakeDataModel→DataModel pointer, each per
// arch. They're edited from the PaymentGate admin dashboard (Offsets section)
// and stored in the `app_data` table under key "offsets". A new Roblox build =
// update 4 fields in the admin, no app reship. Workspace/Camera stay in the app.
const DEFAULT_OFFSETS = {
  fdmArm: 0x77e5d68,
  fdmIntel: 0x77bacc8,
  dmPtrArm: 0x18,
  dmPtrIntel: 0x18,
};
const toNum = (v: unknown, d: number): number => {
  if (v == null || v === "") return d;
  if (typeof v === "number") return Number.isFinite(v) ? v : d;
  const s = String(v).trim();
  const n = s.startsWith("0x") ? parseInt(s, 16) : parseInt(s, 10);
  return Number.isFinite(n) ? n : d;
};
async function loadOffsets() {
  let cfg: Record<string, unknown> = {};
  try {
    const db = createClient(SB_URL, SB_SERVICE, { auth: { persistSession: false } });
    const { data } = await db.from("app_data").select("value").eq("key", "offsets").maybeSingle();
    if (data && data.value && typeof data.value === "object") cfg = data.value as Record<string, unknown>;
  } catch {
    cfg = {};
  }
  return {
    fdmArm: toNum(cfg.fdmArm, DEFAULT_OFFSETS.fdmArm),
    fdmIntel: toNum(cfg.fdmIntel, DEFAULT_OFFSETS.fdmIntel),
    dmPtrArm: toNum(cfg.dmPtrArm, DEFAULT_OFFSETS.dmPtrArm),
    dmPtrIntel: toNum(cfg.dmPtrIntel, DEFAULT_OFFSETS.dmPtrIntel),
  };
}

function json(o: unknown, status = 200): Response {
  return new Response(JSON.stringify(o), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

// hex(HMAC-SHA256(secret, "{ts}.{rawBody}")) — mirrors the app's `sign()`.
async function hmacHex(secret: string, msg: string): Promise<string> {
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const buf = await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(msg));
  return [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

function safeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}

function hex(bytes: Uint8Array): string {
  return [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
}
function unhex(s: string): Uint8Array {
  const m = s.match(/../g);
  return m ? new Uint8Array(m.map((h) => parseInt(h, 16))) : new Uint8Array();
}

// Seal `plaintext` to the app's ephemeral X25519 public key so it can't be read
// on the wire (defence in depth over TLS). shared = X25519(serverPriv, cpub);
// key = SHA-256(shared); AES-256-GCM. Returns hex { encPub, iv, ct }.
async function seal(cpub: Uint8Array, plaintext: string) {
  const sPriv = x25519.utils.randomPrivateKey();
  const sPub = x25519.getPublicKey(sPriv);
  const shared = x25519.getSharedSecret(sPriv, cpub);
  const key = new Uint8Array(await crypto.subtle.digest("SHA-256", shared));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const ck = await crypto.subtle.importKey("raw", key, { name: "AES-GCM" }, false, ["encrypt"]);
  const ct = new Uint8Array(
    await crypto.subtle.encrypt({ name: "AES-GCM", iv }, ck, new TextEncoder().encode(plaintext)),
  );
  return { encPub: hex(sPub), iv: hex(iv), ct: hex(ct) };
}

// A single KeyAuth form POST, with the same response-signature verification the
// app used to do (HMAC over the body keyed by `sigKey`), so a MITM'd KeyAuth
// response is rejected here just as it was in the client.
async function keyauthPost(
  form: Record<string, string>,
  sigKey: string,
): Promise<Record<string, unknown> | null> {
  const resp = await fetch(KEYAUTH_URL, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams(form).toString(),
  });
  const sig = resp.headers.get("signature");
  const bodyText = await resp.text();
  if (!sig) return null;
  if (!safeEqual(await hmacHex(sigKey, bodyText), sig)) return null;
  try {
    return JSON.parse(bodyText);
  } catch {
    return null;
  }
}

// Version gate — the admin (PaymentGate "Cloud" section) keeps a list of build
// hashes and which are disabled. A client on a disabled build is force-updated:
// the app shows "Socrate new version: <latest>" and quits.
async function loadVersions(): Promise<{ latest: string; disabled: string[] }> {
  try {
    const db = createClient(SB_URL, SB_SERVICE, { auth: { persistSession: false } });
    const { data } = await db.from("app_data").select("value").eq("key", "versions").maybeSingle();
    const v = (data && data.value) as
      | { latest?: string; versions?: Array<{ hash?: string; disabled?: boolean }> }
      | undefined;
    if (!v) return { latest: "", disabled: [] };
    const disabled = (v.versions || [])
      .filter((x) => x && x.disabled && x.hash)
      .map((x) => String(x.hash));
    return { latest: String(v.latest || ""), disabled };
  } catch {
    return { latest: "", disabled: [] };
  }
}

Deno.serve(async (req) => {
  if (req.method !== "POST") return json({ error: "method" }, 405);
  if (!ED_PRIV) return json({ error: "server misconfigured" }, 500);

  const raw = await req.text();

  // Inbound request auth — same signing scheme as `access`.
  if (HMAC_SECRET) {
    const ts = req.headers.get("x-socrate-ts") ?? "";
    const sig = req.headers.get("x-socrate-sig") ?? "";
    const tsNum = parseInt(ts, 10);
    if (!ts || !sig || !Number.isFinite(tsNum)) return json({ error: "unsigned" }, 401);
    if (Math.abs(Math.floor(Date.now() / 1000) - tsNum) > MAX_SKEW) {
      return json({ error: "stale" }, 401);
    }
    if (!safeEqual(await hmacHex(HMAC_SECRET, `${ts}.${raw}`), sig)) {
      return json({ error: "bad signature" }, 401);
    }
  } else {
    console.warn("SOCRATE_HMAC_SECRET not set — request signatures are NOT verified");
  }

  let body: Record<string, unknown>;
  try {
    body = JSON.parse(raw);
  } catch {
    return json({ error: "bad json" }, 400);
  }

  const key = String(body.key ?? "").trim();
  const hwid = String(body.hwid ?? "").trim();
  const nonce = String(body.nonce ?? "").trim();
  const arch = String(body.arch ?? "").toLowerCase(); // "arm" | "intel"
  const build = String(body.build ?? "").trim(); // this binary's version hash

  // ─── Pre-launch version check (keyless) ───
  // Socrate calls this before it launches; no licence needed, just the build
  // hash. Returns a signed blocked/latest so a disabled build never starts.
  if (body.vcheck) {
    if (!nonce) return json({ error: "missing fields" }, 400);
    const vers = await loadVersions();
    const blocked = !!build && vers.disabled.includes(build);
    const latest = vers.latest || "";
    const iat = Math.floor(Date.now() / 1000);
    const msg = `vcheck|${nonce}|${blocked ? 1 : 0}|${latest}|${iat}`;
    const sig = hex(await ed.signAsync(new TextEncoder().encode(msg), ED_PRIV));
    return json({ vcheck: true, blocked, latest, nonce, iat, sig });
  }

  if (!key || !hwid || !nonce) return json({ error: "missing fields" }, 400);

  // ─── KeyAuth exchange (init → license), entirely server-side ───
  const enckey = crypto.randomUUID().replace(/-/g, "");
  const init = await keyauthPost(
    { type: "init", ver: KEYAUTH_VER, name: KEYAUTH_NAME, ownerid: KEYAUTH_OWNER, enckey },
    KEYAUTH_SECRET,
  );
  if (!init || init.success !== true || typeof init.sessionid !== "string") {
    return json({ error: "keyauth init failed" }, 502);
  }

  const license = await keyauthPost(
    {
      type: "license",
      key,
      hwid,
      sessionid: init.sessionid as string,
      name: KEYAUTH_NAME,
      ownerid: KEYAUTH_OWNER,
    },
    `${enckey}-${KEYAUTH_SECRET}`,
  );
  if (!license) return json({ error: "keyauth license failed" }, 502);

  const valid = license.success === true;
  // expiry: Unix-seconds string ("Never"/unparseable => 0 = lifetime), → millis.
  let expiry = 0;
  if (valid) {
    const info = license.info as { subscriptions?: Array<{ expiry?: string }> } | undefined;
    const raw = info?.subscriptions?.[0]?.expiry;
    const secs = raw ? parseInt(raw, 10) : NaN;
    if (Number.isFinite(secs)) expiry = secs * 1000;
  }

  // ─── Resolve-chain offsets, sealed to the app's ephemeral key ───
  // Only on a valid licence, and only if the app supplied a valid X25519 pubkey.
  const isArm = arch.includes("arm");
  const cpub = unhex(String(body.cpub ?? ""));
  let sealed = { encPub: "", iv: "", ct: "" };
  if (valid && cpub.length === 32) {
    const off = await loadOffsets();
    const fdmEntry = isArm ? off.fdmArm : off.fdmIntel;
    const fdmDataModel = isArm ? off.dmPtrArm : off.dmPtrIntel;
    sealed = await seal(cpub, `${fdmEntry}|${fdmDataModel}`);
  }

  // ─── Version gate ───
  const vers = await loadVersions();
  const blocked = !!build && vers.disabled.includes(build);
  const latest = vers.latest || "";

  // ─── Sign the verdict (sealed blob + version gate are all signed) ───
  const iat = Math.floor(Date.now() / 1000);
  const msg =
    `${nonce}|${valid ? 1 : 0}|${expiry}|${hwid}|${iat}` +
    `|${sealed.encPub}|${sealed.iv}|${sealed.ct}` +
    `|${blocked ? 1 : 0}|${latest}`;
  const sig = hex(await ed.signAsync(new TextEncoder().encode(msg), ED_PRIV));

  return json({ valid, expiry, hwid, nonce, iat, sig, offsets: sealed, blocked, latest });
});
