#!/bin/bash
# Socrate build + publish pipeline.
#
# Usage:
#   ./builder_sender.sh
#
# Prompts for `Updated_for_roblox_v` and the target arch (arm / intel),
# runs `tauri build`, drops the resulting .dmg into the SocrateWebsite
# repo, updates status.json in place, commits and pushes.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOCRATE_DIR="${SCRIPT_DIR}"
WEBSITE_DIR="/Users/admin/Desktop/SocrateWebsite/SocrateWebsite"
REMOTE_RAW_BASE="https://raw.githubusercontent.com/Yop1-BSecretKikx/SocrateWebsite/main"

cyan() { printf "\033[36m%s\033[0m\n" "$*"; }
green(){ printf "\033[32m%s\033[0m\n" "$*"; }
red()  { printf "\033[31m%s\033[0m\n" "$*" >&2; }

die() { red "[X] $1"; exit 1; }
say() { cyan "[*] $1"; }

[ -d "${WEBSITE_DIR}" ]      || die "SocrateWebsite not found at ${WEBSITE_DIR}"
[ -d "${SOCRATE_DIR}/src-tauri" ] || die "Run this from the Socrate project root."
command -v jq >/dev/null 2>&1     || die "jq is required (brew install jq)."

# ─── Prompts ────────────────────────────────────────────────────────
read -r -p "Updated_for_roblox_v: " ROBLOX_V
[ -z "${ROBLOX_V}" ] && die "version required"

echo ""
echo "arch:"
echo "  1) arm (Apple Silicon)"
echo "  2) intel (x86_64)"
read -r -p "choice (1/2): " ARCH_CHOICE

case "${ARCH_CHOICE}" in
    1) TARGET="aarch64-apple-darwin"; ARCH_LABEL="arm"   ;;
    2) TARGET="x86_64-apple-darwin";  ARCH_LABEL="intel" ;;
    *) die "invalid choice" ;;
esac

if ! rustup target list --installed 2>/dev/null | grep -q "^${TARGET}$"; then
    say "Installing rustup target ${TARGET}..."
    rustup target add "${TARGET}"
fi

# ─── Build ──────────────────────────────────────────────────────────
cd "${SOCRATE_DIR}"
say "Building for ${TARGET}..."
npm run tauri build -- --target "${TARGET}"

DMG_DIR="src-tauri/target/${TARGET}/release/bundle/dmg"
DMG_SRC="$(ls -1t "${DMG_DIR}"/*.dmg 2>/dev/null | head -1 || true)"
[ -n "${DMG_SRC}" ] && [ -f "${DMG_SRC}" ] \
    || die "DMG not found in ${DMG_DIR}"
say "DMG produced: ${DMG_SRC}"

DMG_NAME="socrate-${ARCH_LABEL}.dmg"
cp "${DMG_SRC}" "${WEBSITE_DIR}/${DMG_NAME}"
say "Copied to ${WEBSITE_DIR}/${DMG_NAME}"

# ─── status.json ────────────────────────────────────────────────────
# Keep the schema the launcher already reads:
#   { "Socrate": { "Updated_for_roblox_v": "...", "arm": "...", "intel": "..." } }
STATUS_JSON="${WEBSITE_DIR}/status.json"
DMG_URL="${REMOTE_RAW_BASE}/${DMG_NAME}"

if [ ! -f "${STATUS_JSON}" ]; then
    cat > "${STATUS_JSON}" <<'EOF'
{ "Socrate": { "Updated_for_roblox_v": "Not Available", "arm": "Not Available", "intel": "Not Available" } }
EOF
fi

TMP="$(mktemp)"
jq --arg v "${ROBLOX_V}" \
   --arg a "${ARCH_LABEL}" \
   --arg u "${DMG_URL}" \
   '.Socrate.Updated_for_roblox_v = $v
    | .Socrate[$a] = $u' \
   "${STATUS_JSON}" > "${TMP}" && mv "${TMP}" "${STATUS_JSON}"
say "status.json updated:"
jq . "${STATUS_JSON}"

# ─── git commit + push ──────────────────────────────────────────────
cd "${WEBSITE_DIR}"
git add "${DMG_NAME}" status.json

if git diff --cached --quiet; then
    say "No changes to commit."
else
    git commit -m "Release ${ARCH_LABEL} build for ${ROBLOX_V}"
    say "Pushing to origin..."
    git push origin main
    green "[*] Done — ${DMG_NAME} live at ${DMG_URL}"
fi
