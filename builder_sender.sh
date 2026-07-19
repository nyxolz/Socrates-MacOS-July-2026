#!/bin/bash
# Socrate build + publish pipeline.
#
# Usage:
#   ./builder_sender.sh
#
# Prompts for `Updated_for_roblox_v`, builds the app for BOTH ARM and Intel,
# copies the ARM .dmg to the SocrateWebsite root and the Intel .dmg into
# `SocrateWebsite/intel/`, updates status.json with both download URLs, then
# commits and pushes in a single commit.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOCRATE_DIR="${SCRIPT_DIR}"

# ─── Deploy targets ─────────────────────────────────────────────────
# `main`  → production: pushes to SocrateWebsite (Yop1-BSecretKikx), URLs on usesocrate.xyz.
# `prov`  → provisional/preview: pushes to `not_default_socrate_repo` (pleglou26-oss),
#           DMGs + launcher served from raw.githubusercontent.com.

MAIN_WEBSITE_DIR="/Users/admin/Desktop/SocrateWebsite/SocrateWebsite"
MAIN_RAW_BASE="https://raw.githubusercontent.com/Yop1-BSecretKikx/SocrateWebsite/main"

PROV_WEBSITE_DIR="/Users/admin/Desktop/not_default_socrate_repo"
PROV_GIT_URL="https://github.com/pleglou26-oss/not_default_socrate_repo.git"
PROV_RAW_BASE="https://raw.githubusercontent.com/pleglou26-oss/not_default_socrate_repo/main"

cyan() { printf "\033[36m%s\033[0m\n" "$*"; }
green(){ printf "\033[32m%s\033[0m\n" "$*"; }
red()  { printf "\033[31m%s\033[0m\n" "$*" >&2; }

die() { red "[X] $1"; exit 1; }
say() { cyan "[*] $1"; }

[ -d "${SOCRATE_DIR}/src-tauri" ]  || die "Run this from the Socrate project root."
command -v jq >/dev/null 2>&1       || die "jq is required (brew install jq)."

# ─── Prompts ────────────────────────────────────────────────────────
# Which repo to publish to. Every downstream URL/path is derived from this.
read -r -p "Send to main or prov? [main/prov]: " DEPLOY_TARGET
DEPLOY_TARGET="$(echo "${DEPLOY_TARGET}" | tr '[:upper:]' '[:lower:]')"
case "${DEPLOY_TARGET}" in
    main)
        WEBSITE_DIR="${MAIN_WEBSITE_DIR}"
        REMOTE_RAW_BASE="${MAIN_RAW_BASE}"
        [ -d "${WEBSITE_DIR}" ] || die "SocrateWebsite not found at ${WEBSITE_DIR}"
        say "Target: MAIN → ${WEBSITE_DIR}"
        ;;
    prov)
        WEBSITE_DIR="${PROV_WEBSITE_DIR}"
        REMOTE_RAW_BASE="${PROV_RAW_BASE}"
        if [ ! -d "${WEBSITE_DIR}/.git" ]; then
            say "Cloning ${PROV_GIT_URL} into ${WEBSITE_DIR}..."
            git clone "${PROV_GIT_URL}" "${WEBSITE_DIR}" || die "clone failed"
        fi
        say "Target: PROV → ${WEBSITE_DIR}"
        ;;
    *)
        die "invalid target '${DEPLOY_TARGET}' (expected 'main' or 'prov')"
        ;;
esac

read -r -p "Updated_for_roblox_v: " ROBLOX_V
[ -z "${ROBLOX_V}" ] && die "version required"

read -r -p "scorate (ex: Up): " SCORATE_STATUS
[ -z "${SCORATE_STATUS}" ] && die "scorate status required"

read -r -p "Whats_New: " WHATS_NEW

# Build matrix — order matters only for display.
TARGETS=( "aarch64-apple-darwin" "x86_64-apple-darwin" )
LABELS=(  "arm"                  "intel"               )

# Install missing rustup targets up front so a build doesn't stall halfway
# through asking for one.
for t in "${TARGETS[@]}"; do
    if ! rustup target list --installed 2>/dev/null | grep -q "^${t}$"; then
        say "Installing rustup target ${t}..."
        rustup target add "${t}"
    fi
done

generate_hash() {
  LC_ALL=C tr -dc 'A-Za-z0-9' < /dev/urandom | head -c 10
  echo
}

# Stamp the version BEFORE compiling so it ends up baked into both binaries.
# Same hash for both archs — they represent the same release.
SOCRATE_VERSION="$(generate_hash)"
VERSION_RS="${SOCRATE_DIR}/src-tauri/src/version.rs"
[ -f "${VERSION_RS}" ] || die "src-tauri/src/version.rs missing"
sed -i.bak -E "s|^pub const VERSION: &str = .*$|pub const VERSION: \&str = \"${SOCRATE_VERSION}\";|" "${VERSION_RS}"
rm -f "${VERSION_RS}.bak"
say "Baked VERSION=${SOCRATE_VERSION} into version.rs"

# STATUS_URL is deploy-target-dependent — main hits usesocrate.xyz, prov hits
# the raw GitHub URL. The binary checks this endpoint for the "update
# available" banner, so it must match wherever the DMG was actually shipped.
if [ "${DEPLOY_TARGET}" = "prov" ]; then
    STATUS_URL_BAKED="https://raw.githubusercontent.com/pleglou26-oss/not_default_socrate_repo/refs/heads/main/status.json"
else
    STATUS_URL_BAKED="https://usesocrate.xyz/status.json"
fi
sed -i.bak -E "s|^pub const STATUS_URL: &str = .*$|pub const STATUS_URL: \&str = \"${STATUS_URL_BAKED}\";|" "${VERSION_RS}"
rm -f "${VERSION_RS}.bak"
say "Baked STATUS_URL=${STATUS_URL_BAKED} into version.rs"

# ─── Inject Supabase secrets (obfuscated, never committed) ───────────
# Values come from .env.build (gitignored) and are written into secrets.rs
# wrapped in obfstr! so the literals are XOR-scrambled in the binary. Only the
# publishable key + URL ever ship — the service_role key lives server-side in
# the Edge Function, so there is no powerful secret to recover from the app.
SECRETS_RS="${SOCRATE_DIR}/src-tauri/src/secrets.rs"
ENV_BUILD="${SOCRATE_DIR}/.env.build"
[ -f "${ENV_BUILD}" ] || die ".env.build missing (copy .env.build.example and fill it in)"
set -a; . "${ENV_BUILD}"; set +a
: "${SUPABASE_URL:?Set SUPABASE_URL in .env.build}"
: "${SUPABASE_PUBLISHABLE_KEY:?Set SUPABASE_PUBLISHABLE_KEY in .env.build}"
: "${SOCRATE_HMAC_SECRET:?Set SOCRATE_HMAC_SECRET in .env.build (must match the Supabase secret)}"
# KeyAuth credentials are NO LONGER embedded — they live in the key-validate
# Edge Function's env. The app only embeds the Ed25519 PUBLIC key used to verify
# server-signed license verdicts (the private key stays in Supabase secrets).
: "${SOCRATE_ED25519_PUB:?Set SOCRATE_ED25519_PUB in .env.build (64 hex chars; public half of SOCRATE_ED25519_PRIV)}"
case "${SUPABASE_PUBLISHABLE_KEY}" in
    sb_publishable_*|eyJ*) : ;;
    *) die "SUPABASE_PUBLISHABLE_KEY must be a publishable/anon key, not service_role" ;;
esac
cat > "${SECRETS_RS}" <<EOF
//! AUTO-GENERATED by builder_sender.sh — do not edit, do not commit.
use obfstr::obfstr;

pub fn supabase_url() -> String {
    obfstr!("${SUPABASE_URL}").to_owned()
}

pub fn supabase_anon_key() -> String {
    obfstr!("${SUPABASE_PUBLISHABLE_KEY}").to_owned()
}

pub fn supabase_hmac_secret() -> String {
    obfstr!("${SOCRATE_HMAC_SECRET}").to_owned()
}

pub fn keyauth_ed25519_pub() -> String {
    obfstr!("${SOCRATE_ED25519_PUB}").to_owned()
}
EOF
say "Injected obfuscated Supabase secrets + Ed25519 public key into secrets.rs"

# ─── Live progress wrapper ──────────────────────────────────────────
# Runs `tauri build` in the background, redirects its (very noisy) output
# to a log, and prints a single self-rewriting status line: spinner +
# elapsed time + crates compiled so far + name of the current crate.
# On success: replaces the line with a one-line summary.
# On failure: prints the tail of the log and exits non-zero.
build_with_progress() {
    local target="$1"
    local label="$2"
    local log
    log="$(mktemp -t "socrate-build-${label}.XXXXXX")"

    # Pre-clean SvelteKit output between builds — its `rimraf` races with
    # macOS Finder/Spotlight on .svelte-kit/output and throws ENOTEMPTY.
    rm -rf .svelte-kit/output build

    npm run tauri build -- --target "${target}" >"${log}" 2>&1 &
    local pid=$!

    local frames=( '⠋' '⠙' '⠹' '⠸' '⠼' '⠴' '⠦' '⠧' '⠇' '⠏' )
    local i=0
    local start
    start=$(date +%s)

    # Re-print every ~120ms so the spinner looks smooth without hammering grep.
    while kill -0 "$pid" 2>/dev/null; do
        local elapsed=$(( $(date +%s) - start ))
        local compiled
        compiled=$(grep -c "^[[:space:]]*Compiling " "${log}" 2>/dev/null || true)
        compiled=${compiled:-0}
        local last
        last=$(grep "^[[:space:]]*Compiling " "${log}" | tail -1 \
               | sed -E 's/^[[:space:]]*Compiling[[:space:]]+([^[:space:]]+).*/\1/' \
               || true)
        printf "\r\033[K  %s build %-5s  %3ds  [%3d crates]  %s" \
               "${frames[$((i % 10))]}" "${label}" "${elapsed}" "${compiled}" "${last:-…}"
        i=$((i + 1))
        sleep 0.12
    done

    # Reap and check status without tripping `set -e` ourselves.
    set +e
    wait "$pid"
    local rc=$?
    set -e

    local elapsed=$(( $(date +%s) - start ))
    if [ "$rc" -eq 0 ]; then
        printf "\r\033[K  \033[32m✓\033[0m %-5s built in %ds\n" "${label}" "${elapsed}"
        rm -f "${log}"
    else
        printf "\r\033[K  \033[31m✗\033[0m %-5s build failed (rc=%d) — last 40 lines:\n" "${label}" "${rc}"
        tail -40 "${log}" >&2
        rm -f "${log}"
        exit "$rc"
    fi
}

cd "${SOCRATE_DIR}"

# ─── Build both archs ───────────────────────────────────────────────
say "Building ARM + Intel..."
for idx in 0 1; do
    build_with_progress "${TARGETS[$idx]}" "${LABELS[$idx]}"
done

# ─── Locate, copy, build URLs ───────────────────────────────────────
# ARM lives at the SocrateWebsite root; Intel goes under intel/ as requested.
ARM_DMG_SRC="$(ls -1t "src-tauri/target/${TARGETS[0]}/release/bundle/dmg"/*.dmg 2>/dev/null | head -1 || true)"
INTEL_DMG_SRC="$(ls -1t "src-tauri/target/${TARGETS[1]}/release/bundle/dmg"/*.dmg 2>/dev/null | head -1 || true)"
[ -n "${ARM_DMG_SRC}"   ] && [ -f "${ARM_DMG_SRC}"   ] || die "ARM dmg not produced"
[ -n "${INTEL_DMG_SRC}" ] && [ -f "${INTEL_DMG_SRC}" ] || die "Intel dmg not produced"

ARM_DMG_NAME="socrate-arm.dmg"
INTEL_DMG_NAME="socrate-intel.dmg"
INTEL_SUBDIR="intel"

mkdir -p "${WEBSITE_DIR}/${INTEL_SUBDIR}"
cp "${ARM_DMG_SRC}"   "${WEBSITE_DIR}/${ARM_DMG_NAME}"
cp "${INTEL_DMG_SRC}" "${WEBSITE_DIR}/${INTEL_SUBDIR}/${INTEL_DMG_NAME}"
say "Copied DMGs:"
say "  ARM   → ${WEBSITE_DIR}/${ARM_DMG_NAME}"
say "  Intel → ${WEBSITE_DIR}/${INTEL_SUBDIR}/${INTEL_DMG_NAME}"

ARM_DMG_URL="${REMOTE_RAW_BASE}/${ARM_DMG_NAME}"
INTEL_DMG_URL="${REMOTE_RAW_BASE}/${INTEL_SUBDIR}/${INTEL_DMG_NAME}"

# ─── status.json ────────────────────────────────────────────────────
# Schema the launcher already reads:
#   { "Socrate": { "Updated_for_roblox_v", "arm", "intel", "scorate",
#                  "socrate_version", "Whats_New" } }
STATUS_JSON="${WEBSITE_DIR}/status.json"
if [ ! -f "${STATUS_JSON}" ]; then
    cat > "${STATUS_JSON}" <<'EOF'
{ "Socrate": { "Updated_for_roblox_v": "Not Available", "arm": "Not Available", "intel": "Not Available", "scorate": "Not Available", "socrate_version": "Not Available", "Whats_New": "" } }
EOF
fi

TMP="$(mktemp)"
jq --arg v   "${ROBLOX_V}" \
   --arg ua  "${ARM_DMG_URL}" \
   --arg ui  "${INTEL_DMG_URL}" \
   --arg s   "${SCORATE_STATUS}" \
   --arg sv  "${SOCRATE_VERSION}" \
   --arg wn  "${WHATS_NEW}" \
   '.Socrate.Updated_for_roblox_v = $v
    | .Socrate.arm   = $ua
    | .Socrate.intel = $ui
    | .Socrate.scorate = $s
    | .Socrate.socrate_version = $sv
    | .Socrate.Whats_New = $wn' \
   "${STATUS_JSON}" > "${TMP}" && mv "${TMP}" "${STATUS_JSON}"
say "status.json updated:"
jq . "${STATUS_JSON}"

# ─── PROV: emit the launcher.sh alongside the DMGs ─────────────────
# The launcher normally lives on the main repo and hits usesocrate.xyz. For
# the provisional repo we ship it *inside* the repo (as `luncher.sh`) with
# every URL rewritten to raw.githubusercontent.com/<prov repo>. This lets
# someone install a preview build with:
#   curl -fsSL <PROV_RAW_BASE>/luncher.sh | bash
PROV_LUNCHER_FILE=""
if [ "${DEPLOY_TARGET}" = "prov" ]; then
    PROV_LUNCHER_FILE="luncher.sh"
    STATUS_URL="${PROV_RAW_BASE}/status.json"
    ENTITLEMENTS_URL_PROV="${PROV_RAW_BASE}/entitlements.plist"
    # If the caller placed an entitlements.plist alongside builder_sender.sh
    # (or inside src-tauri/), copy it into the repo so the launcher can fetch it.
    for cand in "${SOCRATE_DIR}/entitlements.plist" "${SOCRATE_DIR}/src-tauri/entitlements.plist"; do
        if [ -f "${cand}" ]; then
            cp "${cand}" "${WEBSITE_DIR}/entitlements.plist"
            say "Copied entitlements.plist → ${WEBSITE_DIR}/"
            break
        fi
    done
    cat > "${WEBSITE_DIR}/${PROV_LUNCHER_FILE}" <<LUNCHER_EOF
#!/bin/bash

set -u

C_RESET="\033[0m"
C_DIM="\033[2m"
C_BOLD="\033[1m"
C_GREEN="\033[32m"
C_RED="\033[31m"
C_YELLOW="\033[33m"
C_CYAN="\033[36m"
C_GRAY="\033[90m"

get_time() {
    local s=""
    local mot="Socrate"
    local couleurs=("255;235;100" "255;215;0" "255;195;0" "230;170;0" "210;140;0" "190;120;0" "170;100;0")
    for ((i=0; i<\${#mot}; i++)); do
        s+="\033[38;2;\${couleurs[\$i]}m\${mot:\$i:1}"
    done
    s+="\${C_RESET}"
    printf "%b" "\${s}\${C_GRAY}::\${C_RESET}\${C_GREEN}[\$(date +%H:%M:%S)]\${C_RESET}"
}

die() {
    spinner_stop "fail" "\$1"
    exit 1
}


SPIN_FRAMES=("⣾" "⣽" "⣻" "⢿" "⡿" "⣟" "⣯" "⣷")
SPINNER_PID=""
SPINNER_MSG=""

spinner_start() {
    SPINNER_MSG="\$1"
    printf "\033[?25l\033[?7l"
    (
        local i=0
        while true; do
            local frame="\${SPIN_FRAMES[\$((i % \${#SPIN_FRAMES[@]}))]}"
            printf "\r\033[2K%b \${C_CYAN}%s\${C_RESET}  %s" "\$(get_time)" "\$frame" "\$SPINNER_MSG"
            i=\$((i+1))
            sleep 0.08
        done
    ) &
    SPINNER_PID=\$!
    disown "\$SPINNER_PID" 2>/dev/null || true
}

spinner_stop() {
    local status="\${1:-ok}"
    local msg="\${2:-\$SPINNER_MSG}"
    if [[ -n "\$SPINNER_PID" ]] && kill -0 "\$SPINNER_PID" 2>/dev/null; then
        kill "\$SPINNER_PID" 2>/dev/null
        wait "\$SPINNER_PID" 2>/dev/null || true
    fi
    SPINNER_PID=""
    printf "\r\033[2K"
    case "\$status" in
        ok)   printf "%b \${C_GREEN}✔\${C_RESET}  %b\n" "\$(get_time)" "\$msg" ;;
        fail) printf "%b \${C_RED}✖\${C_RESET}  %b\n"   "\$(get_time)" "\$msg" ;;
        warn) printf "%b \${C_YELLOW}!\${C_RESET}  %b\n" "\$(get_time)" "\$msg" ;;
        *)    printf "%b    %b\n" "\$(get_time)" "\$msg" ;;
    esac
    printf "\033[?7h\033[?25h"
}

download_with_progress() {
    local url="\$1"
    local out="\$2"
    local label="\$3"
    local bar_width=20

    local total
    total=\$(curl -sIL -m 10 "\$url" \
        | awk 'BEGIN{IGNORECASE=1} /^content-length:/ {gsub(/\r/,""); v=\$2} END{print v+0}')
    [[ -z "\$total" || "\$total" == "0" ]] && total=0

    curl -fL -s "\$url" -o "\$out" &
    local pid=\$!

    printf "\033[?25l\033[?7l"

    local i=0
    local min_loops=5
    while kill -0 "\$pid" 2>/dev/null || (( i < min_loops )); do
        local cur=0
        [[ -f "\$out" ]] && cur=\$(stat -f%z "\$out" 2>/dev/null || echo 0)

        local pct="0.0" filled=0
        if [[ "\$total" -gt 0 ]]; then
            pct=\$(awk -v c="\$cur" -v t="\$total" 'BEGIN{p=(c/t)*100; if(p>100)p=100; printf "%.1f", p}')
            filled=\$(awk -v c="\$cur" -v t="\$total" -v w="\$bar_width" 'BEGIN{f=int((c/t)*w); if(f>w)f=w; print f}')
        fi

        local bar=""
        for ((j=0; j<filled; j++));        do bar+="━"; done
        for ((j=filled; j<bar_width; j++)); do bar+="─"; done

        local frame="\${SPIN_FRAMES[\$((i % \${#SPIN_FRAMES[@]}))]}"
        printf "\r\033[2K%b \${C_CYAN}%s\${C_RESET} %s \${C_GREEN}%s\${C_RESET} \${C_BOLD}%5s%%\${C_RESET}" \
            "\$(get_time)" "\$frame" "\$label" "\$bar" "\$pct"
        i=\$((i+1))
        sleep 0.1

        if ! kill -0 "\$pid" 2>/dev/null && (( i >= min_loops )); then
            break
        fi
    done
    wait "\$pid" 2>/dev/null; local rc=\$?

    printf "\r\033[2K"
    if [[ \$rc -eq 0 ]]; then
        local bar=""
        for ((j=0; j<bar_width; j++)); do bar+="━"; done
        printf "%b \${C_GREEN}✔\${C_RESET} %s \${C_GREEN}%s\${C_RESET} \${C_BOLD}100.0%%\${C_RESET}\n" \
            "\$(get_time)" "\$label" "\$bar"
    else
        printf "%b \${C_RED}✖\${C_RESET} %s failed\n" "\$(get_time)" "\$label"
    fi

    printf "\033[?7h\033[?25h"
    return \$rc
}

log() { printf "%b %b\n" "\$(get_time)" "\$1"; }

banner() {
    local line="────────────────────────────────────────────"
    echo ""
    printf "\${C_GRAY}%s\${C_RESET}\n" "\$line"
    printf "  %b  \${C_BOLD}Installer (prov)\${C_RESET}\n" "\$(printf "\033[38;2;255;215;0m%s\033[0m" "Socrate")"
    printf "\${C_GRAY}%s\${C_RESET}\n" "\$line"
    echo ""
}

detect_arch() {
    local os arch
    os=\$(uname -s); arch=\$(uname -m)
    if [[ "\$os" == "Darwin" ]]; then
        if [[ "\$(sysctl -n sysctl.proc_translated 2>/dev/null)" == "1" ]]; then
            arch="arm64"
        fi
    fi
    case "\$arch" in
        arm64|aarch64) echo "arm" ;;
        x86_64|amd64)  echo "intel" ;;
        *)             echo "unknown" ;;
    esac
}

killall -9 Roblox       2>/dev/null
killall -9 RobloxPlayer 2>/dev/null
killall -9 Socrate      2>/dev/null

banner

spinner_start "checking versions..."
MAC_VERSION=\$(curl -fsS -m 8 -H "User-Agent: WEAO-3PService" "https://weao.xyz/api/versions/current" | jq -r '.Mac' 2>/dev/null)
UPDATED_FOR_ROBLOX_V=\$(curl -fsS -m 8 "${STATUS_URL}" | jq -r '.Socrate.Updated_for_roblox_v' 2>/dev/null)
[[ -z "\$MAC_VERSION" || "\$MAC_VERSION" == "null" ]] && MAC_VERSION="unknown"
[[ -z "\$UPDATED_FOR_ROBLOX_V" || "\$UPDATED_FOR_ROBLOX_V" == "null" ]] && UPDATED_FOR_ROBLOX_V="unknown"
spinner_stop ok "versions fetched"

log "latest roblox version : \${C_BOLD}\${MAC_VERSION}\${C_RESET}"
log "socrate updated for   : \${C_BOLD}\${UPDATED_FOR_ROBLOX_V}\${C_RESET}"


if [[ "\$MAC_VERSION" != "unknown" && "\$UPDATED_FOR_ROBLOX_V" != "unknown" && "\$MAC_VERSION" != "\$UPDATED_FOR_ROBLOX_V" ]]; then
    echo ""
    printf "  \${C_YELLOW}⚠  Socrate is OUTDATED\${C_RESET}\n"
    printf "  \${C_GRAY}Roblox is on \${MAC_VERSION} but Socrate supports \${UPDATED_FOR_ROBLOX_V}.\${C_RESET}\n"
    printf "  \${C_GRAY}It may not work until an update is released.\${C_RESET}\n"
    echo ""
    printf "%b " "\$(get_time)"
    read -r -p "Continue anyway? [y/N]: " cont </dev/tty
    case "\$cont" in
        y|Y|yes|YES) log "continuing..." ;;
        *) die "aborted by user" ;;
    esac
else
    log "\${C_GREEN}socrate is up to date\${C_RESET}"
fi


echo ""
log "Choose your installation:"
echo ""
echo "   1) ARM (Apple Silicon)"
echo "   2) Intel"
echo "   3) Auto-detect"
echo ""

archi=""
while [[ -z "\$archi" ]]; do
    printf "%b " "\$(get_time)"
    read -r -p "Enter your choice [1/2/3]: " choice </dev/tty
    case "\$choice" in
        1) archi="arm";   log "you chose ARM" ;;
        2) archi="intel"; log "you chose Intel" ;;
        3)
            spinner_start "detecting architecture..."
            sleep 0.4
            archi=\$(detect_arch)
            if [[ "\$archi" == "arm" ]]; then
                spinner_stop ok "detected ARM (Apple Silicon)"
            elif [[ "\$archi" == "intel" ]]; then
                spinner_stop ok "detected Intel"
            else
                spinner_stop fail "unsupported architecture"
                exit 1
            fi
            ;;
        *) log "\${C_RED}invalid choice\${C_RESET}, please type 1, 2 or 3" ;;
    esac
done

if [[ "\$archi" == "arm" ]]; then
    DMG_URL="${PROV_RAW_BASE}/socrate-arm.dmg"
    ROBLOX_ZIP_URL_BASE="https://setup.rbxcdn.com/mac/arm64"
else
    DMG_URL="${PROV_RAW_BASE}/intel/socrate-intel.dmg"
    ROBLOX_ZIP_URL_BASE="https://setup.rbxcdn.com/mac"
fi

ROBLOX_API="https://clientsettingscdn.roblox.com/v2/client-version/MacPlayer"
ENTITLEMENTS_URL="${ENTITLEMENTS_URL_PROV}"
INSTALL_DIR="/Applications"
ROBLOX_APP="\${INSTALL_DIR}/Roblox.app"

WORKDIR="\$(mktemp -d -t socrate)"
DMG_PATH="\${WORKDIR}/Socrate.dmg"
PLIST_PATH="\${WORKDIR}/entitlement.plist"

MOUNT_PATH=""
SUDO_KEEPALIVE_PID=""

cleanup() {
    [[ -n "\$SPINNER_PID" ]] && kill "\$SPINNER_PID" 2>/dev/null
    printf "\033[?7h\033[?25h"
    [ -n "\${MOUNT_PATH}" ] && [ -d "\${MOUNT_PATH}" ] && hdiutil detach "\${MOUNT_PATH}" -quiet 2>/dev/null
    [ -n "\${SUDO_KEEPALIVE_PID}" ] && kill "\${SUDO_KEEPALIVE_PID}" 2>/dev/null
    rm -rf "\${WORKDIR}"
}
trap cleanup EXIT INT TERM

echo ""
log "\${C_CYAN}admin access required\${C_RESET} — enter your mac password if asked"
sudo -v || die "admin permission denied"
(
    while true; do
        sudo -n true
        sleep 30
        kill -0 "\$\$" 2>/dev/null || exit
    done
) &
SUDO_KEEPALIVE_PID=\$!
disown "\$SUDO_KEEPALIVE_PID" 2>/dev/null || true
log "\${C_GREEN}admin granted\${C_RESET}"

spinner_start "removing old roblox..."
sudo -n killall -9 Roblox 2>/dev/null
sudo -n rm -rf "/Applications/Roblox.app"
sudo -n rm -rf "/Applications/RobloxPlayer.app"
spinner_stop ok "old roblox removed"

spinner_start "fetching roblox build info..."
RO_VERSION=\$(curl -fsS -m 10 "\${ROBLOX_API}" | grep -oE '"clientVersionUpload":"[^"]*"' | cut -d'"' -f4)
[[ -z "\$RO_VERSION" ]] && { spinner_stop fail "could not fetch roblox version"; exit 1; }
spinner_stop ok "roblox build : \${RO_VERSION}"


URL="\${ROBLOX_ZIP_URL_BASE}/\${RO_VERSION}-RobloxPlayer.zip"
OUT="/tmp/RobloxPlayer.zip"
download_with_progress "\$URL" "\$OUT" "downloading roblox " || die "failed downloading roblox"

spinner_start "unpacking roblox..."
cd /tmp || die "cannot cd /tmp"
unzip -o -q "\$OUT" || { spinner_stop fail "unzip failed"; exit 1; }
mv "/tmp/RobloxPlayer.app" "/Applications/Roblox.app"
rm -f "\$OUT"
[ -d "\${ROBLOX_APP}" ] || { spinner_stop fail "roblox install failed"; exit 1; }

sudo -n rm -rf "\${ROBLOX_APP}/Contents/MacOS/RobloxPlayerInstaller.app" 2>/dev/null || true

spinner_stop ok "roblox installed"


spinner_start "preparing roblox..."
sudo -n xattr -cr "\${ROBLOX_APP}" || true
sudo -n codesign --force --sign - "\${ROBLOX_APP}/Contents/MacOS/RobloxPlayer" 2>/dev/null || true
spinner_stop ok "roblox prepared"

download_with_progress "\${DMG_URL}" "\${DMG_PATH}" "downloading socrate" || die "failed downloading dmg"
[ -f "\${DMG_PATH}" ] || die "missing dmg"


download_with_progress "\${ENTITLEMENTS_URL}" "\${PLIST_PATH}" "downloading entitle" || die "failed downloading entitlement"
[ -f "\${PLIST_PATH}" ] || die "missing entitlement"


spinner_start "mounting dmg..."
MOUNT_PATH="\$(hdiutil attach "\${DMG_PATH}" -nobrowse -noautoopen 2>/dev/null | awk -F'\t' '/\/Volumes\// {print \$NF; exit}')"
[ -n "\${MOUNT_PATH}" ] || { spinner_stop fail "mount failed"; exit 1; }
spinner_stop ok "dmg mounted"

APP_NAME="\$(ls "\${MOUNT_PATH}" | grep '\.app\$' | head -n 1)"
[ -n "\${APP_NAME}" ] || die "no app found in dmg"

TARGET_PATH="\${INSTALL_DIR}/\${APP_NAME}"

spinner_start "installing \${APP_NAME}..."
sudo -n rm -rf "\${TARGET_PATH}"
sudo -n cp -R "\${MOUNT_PATH}/\${APP_NAME}" "\${INSTALL_DIR}/" || { spinner_stop fail "install failed"; exit 1; }
hdiutil detach "\${MOUNT_PATH}" -quiet
MOUNT_PATH=""
spinner_stop ok "\${APP_NAME} installed"

spinner_start "removing quarantine..."
sudo -n xattr -rd com.apple.quarantine "\${TARGET_PATH}" 2>/dev/null || true
sudo -n xattr -cr "\${TARGET_PATH}"
spinner_stop ok "quarantine removed"

spinner_start "signing app..."
sudo -n codesign --force --deep --sign - --entitlements "\${PLIST_PATH}" "\${TARGET_PATH}" >/dev/null 2>&1 \
    || { spinner_stop fail "codesign failed"; exit 1; }
spinner_stop ok "app signed"

spinner_start "verifying signature..."
sudo -n codesign --verify --deep "\${TARGET_PATH}" 2>/dev/null
spinner_stop ok "signature verified"


spinner_start "launching roblox..."
open "/Applications/Roblox.app"
sleep 5
spinner_stop ok "roblox launched"

spinner_start "launching socrate..."
sleep 1
open "\${TARGET_PATH}"
spinner_stop ok "socrate launched"

echo ""
printf "  \${C_GREEN}✔  All done — enjoy Socrate (prov build) \${C_RESET}\n"
echo ""
LUNCHER_EOF
    chmod +x "${WEBSITE_DIR}/${PROV_LUNCHER_FILE}"
    say "Wrote provisional launcher → ${WEBSITE_DIR}/${PROV_LUNCHER_FILE}"
fi

# ─── Ship ───────────────────────────────────────────────────────────
# `main`  → git commit + rebase + push (release goes live immediately).
# `prov`  → files are copied into the local repo dir only. Nothing is staged,
#           committed, or pushed. Review the diff yourself, then push
#           manually when ready.
cd "${WEBSITE_DIR}"

if [ "${DEPLOY_TARGET}" = "prov" ]; then
    green "[*] Prov build ready — files placed locally, NO git push."
    green "    Dir   → ${WEBSITE_DIR}"
    green "    ARM   → ${ARM_DMG_NAME}"
    green "    Intel → ${INTEL_SUBDIR}/${INTEL_DMG_NAME}"
    green "    Launcher → ${PROV_LUNCHER_FILE}"
    green "    status.json + entitlements.plist updated in place."
    say "Review the diff, then push manually if it looks good:"
    say "    cd ${WEBSITE_DIR} && git status && git add -A && git commit -m '…' && git push -u origin main"
    exit 0
fi

git add "${ARM_DMG_NAME}" "${INTEL_SUBDIR}/${INTEL_DMG_NAME}" status.json

if git diff --cached --quiet; then
    say "No changes to commit."
else
    git commit -m "Release arm+intel build for ${ROBLOX_V} (${DEPLOY_TARGET})"
    # Skip the rebase if the remote `main` branch doesn't exist yet (fresh
    # repo, first push). Otherwise, edits made directly on GitHub (e.g.
    # luncher.sh) leave the remote ahead and the first push would fail with
    # "fetch first"; rebasing avoids a merge commit.
    if git ls-remote --exit-code --heads origin main >/dev/null 2>&1; then
        say "Syncing with remote (rebase)..."
        git pull --rebase origin main
    else
        say "Remote main not present yet — skipping rebase (first push)."
    fi
    say "Pushing to origin..."
    git push -u origin main
    green "[*] Done — both builds live (${DEPLOY_TARGET}):"
    green "    ARM   → ${ARM_DMG_URL}"
    green "    Intel → ${INTEL_DMG_URL}"
fi
