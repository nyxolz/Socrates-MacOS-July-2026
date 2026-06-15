// Socrate access-control Edge Function.
//
// Holds the powerful service_role key (injected by Supabase as an env var —
// NEVER shipped in the app) and is the ONLY thing allowed to read/write the
// `userbase` table (anon has no direct table access; see lockdown SQL).
//
// The client (the app) only sends a pc_id and asks for a decision. Critically
// the function never sets is_allowed=true from client input — a blacklisted
// machine stays blacklisted no matter what the client sends. That makes the
// blacklist impossible to bypass by reversing the app.
//
// Protocol (POST JSON):
//   { action:"gate",      pc_id, pc_name, country } -> { allowed, total }
//   { action:"heartbeat", pc_id, seconds }          -> { allowed }
//   { action:"stop",      pc_id }                    -> { ok:true }
//   { action:"tamper",    pc_id }                    -> { allowed:false }
//
// `is_playing` tracks live usage: set true on gate/heartbeat, false on stop
// (sent best-effort when the app closes) and on tamper.

import { createClient } from "jsr:@supabase/supabase-js@2";

const URL = Deno.env.get("SUPABASE_URL")!;
const SERVICE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!;
// Shared HMAC secret — must match secrets::supabase_hmac_secret() in the app.
// Set it with: supabase secrets set SOCRATE_HMAC_SECRET=<same value as .env.build>
const HMAC_SECRET = Deno.env.get("SOCRATE_HMAC_SECRET") ?? "";
const TABLE = "userbase";

// Max age (seconds) we accept for a signed request, to bound replay.
const MAX_SKEW = 120;
// Soft rate limit per (pc_id, ip): heartbeat is 1/min so this is generous.
const RL_LIMIT = 20;
const RL_WINDOW_MS = 60_000;

function json(o: unknown, status = 200): Response {
  return new Response(JSON.stringify(o), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

// ─── Request signing (#3) ───────────────────────────────────────────
// hex(HMAC-SHA256(secret, "{ts}.{rawBody}")). Stops anyone who only copied the
// public anon key: they'd also need to reproduce this scheme with the secret.
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

// Length-checked constant-time string compare.
function safeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}

// ─── pc_id validation (#1) ──────────────────────────────────────────
// macOS IOPlatformUUID is a standard UUID; reject anything else to kill spam.
// Ronin tags its ids with a leading "Ronin-" so they're distinguishable in the
// table — allow that optional prefix in addition to the bare Socrate UUID.
const UUID_RE = /^(?:Ronin-)?[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}$/;

// ─── Rate limiting (#2) ─────────────────────────────────────────────
// In-memory fixed window. Note: state is per warm isolate, so this is a soft
// cap (it resets on cold starts and isn't shared across instances) — enough to
// blunt floods, not a hard quota. Move to a table/KV if you need strict limits.
const RL = new Map<string, { count: number; reset: number }>();
function rateLimited(key: string): boolean {
  const now = Date.now();
  const e = RL.get(key);
  if (!e || now > e.reset) {
    RL.set(key, { count: 1, reset: now + RL_WINDOW_MS });
    return false;
  }
  e.count++;
  return e.count > RL_LIMIT;
}

Deno.serve(async (req) => {
  if (req.method !== "POST") return json({ error: "method" }, 405);

  // Read the raw body once: we must hash the exact bytes the client signed.
  const raw = await req.text();

  // Verify the signature before doing anything else. When no secret is
  // configured server-side we skip (lets you deploy before rolling the key),
  // but that path is logged so it doesn't stay unprotected silently.
  if (HMAC_SECRET) {
    const ts = req.headers.get("x-ronin-ts") ?? req.headers.get("x-socrate-ts") ?? "";
    const sig = req.headers.get("x-ronin-sig") ?? req.headers.get("x-socrate-sig") ?? "";
    const tsNum = parseInt(ts, 10);
    if (!ts || !sig || !Number.isFinite(tsNum)) {
      return json({ error: "unsigned" }, 401);
    }
    if (Math.abs(Math.floor(Date.now() / 1000) - tsNum) > MAX_SKEW) {
      return json({ error: "stale" }, 401);
    }
    const expected = await hmacHex(HMAC_SECRET, `${ts}.${raw}`);
    if (!safeEqual(expected, sig)) {
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

  const action = String(body.action ?? "");
  const pc_id = String(body.pc_id ?? "");
  if (!action || !pc_id) return json({ error: "missing fields" }, 400);
  if (!UUID_RE.test(pc_id)) return json({ error: "bad pc_id" }, 400);

  const ip = req.headers.get("x-forwarded-for")?.split(",")[0].trim() ?? "?";
  if (rateLimited(`${pc_id}|${ip}`)) return json({ error: "rate limited" }, 429);

  const db = createClient(URL, SERVICE_KEY, { auth: { persistSession: false } });

  // Tamper report: permanent blacklist. (Only ever sets false.)
  if (action === "tamper") {
    await db.from(TABLE).update({ is_allowed: false, is_playing: false }).eq("pc_id", pc_id);
    return json({ allowed: false });
  }

  // Stop beacon: app is closing — mark the machine as no longer playing.
  if (action === "stop") {
    await db.from(TABLE).update({ is_playing: false }).eq("pc_id", pc_id);
    return json({ ok: true });
  }

  const { data: rows } = await db
    .from(TABLE)
    .select("is_allowed,totale_time")
    .eq("pc_id", pc_id)
    .limit(1);
  const row = rows?.[0] as
    | { is_allowed: boolean | null; totale_time: string | null }
    | undefined;

  if (action === "gate") {
    const pc_name = String(body.pc_name ?? "");
    const country = String(body.country ?? "");
    if (!row) {
      // First launch on this machine — register as allowed.
      await db.from(TABLE).insert({
        pc_id,
        pc_name,
        totale_time: "0",
        user_country: country,
        is_allowed: true,
        is_playing: true,
      });
      return json({ allowed: true, total: 0 });
    }
    // Refresh human-readable fields; decision comes from the stored flag only.
    await db.from(TABLE).update({ pc_name, user_country: country, is_playing: true }).eq("pc_id", pc_id);
    return json({
      allowed: row.is_allowed !== false,
      total: parseInt(row.totale_time ?? "0") || 0,
    });
  }

  if (action === "heartbeat") {
    if (!row) return json({ allowed: true });
    const seconds = Math.max(0, parseInt(String(body.seconds ?? "0")) || 0);
    await db.from(TABLE).update({ totale_time: String(seconds), is_playing: true }).eq("pc_id", pc_id);
    return json({ allowed: row.is_allowed !== false });
  }

  return json({ error: "unknown action" }, 400);
});
