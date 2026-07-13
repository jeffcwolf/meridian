#!/usr/bin/env bash
#
# deploy.sh — build Meridian for the server's architecture, push it to the
# Scaleway registry, and roll just this service in the shared compose stack.
#
# Meridian is a long-running Leptos/Axum server (it binds a TCP port and serves
# SSR HTML + its own static assets + /export/* routes), so this is a
# Pattern A "containerized app" deploy: build image -> push -> `compose up -d`.
#
# This script deliberately does NOT touch Caddy or the compose file — those live
# in the scaleway-infra repo. See the block printed at the end for the one-time
# wiring to paste there.
#
# Overridable via environment:
#   DEPLOY_HOST   ssh target                    (default wolf@51.158.67.158)
#   REGISTRY      container registry host       (default rg.fr-par.scw.cloud)
#   NAMESPACE     registry namespace            (default discrepancies)
#   APP           app / image / service name    (default meridian)
#   DOMAIN        base domain                   (default discrepancies.eu)
#   TAG           rolling tag to publish        (default latest)
#   STACK_DIR     compose stack dir on server   (default /home/wolf/stack)
#   PLATFORM      image platform                (default linux/amd64)
#   ALLOW_DIRTY   set to 1 to skip the clean-tree check (not recommended)
#
set -euo pipefail

# Always operate from the repo root regardless of where the script is invoked.
cd "$(git rev-parse --show-toplevel)"

HOST="${DEPLOY_HOST:-wolf@51.158.67.158}"
REGISTRY="${REGISTRY:-rg.fr-par.scw.cloud}"
NAMESPACE="${NAMESPACE:-discrepancies}"
APP="${APP:-meridian}"
DOMAIN="${DOMAIN:-discrepancies.eu}"
TAG="${TAG:-latest}"
STACK_DIR="${STACK_DIR:-/home/wolf/stack}"
PLATFORM="${PLATFORM:-linux/amd64}"

IMAGE="${REGISTRY}/${NAMESPACE}/${APP}"
URL="https://${APP}.${DOMAIN}"

log() { printf '\n\033[1;34m==>\033[0m %s\n' "$*"; }
die() { printf '\n\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

# --- Preconditions ----------------------------------------------------------
command -v docker >/dev/null 2>&1 || die "docker not found on PATH."

# A deploy must map to a real commit, so refuse to ship a dirty tree.
if [ "${ALLOW_DIRTY:-0}" != "1" ] && [ -n "$(git status --porcelain)" ]; then
  die "working tree is dirty. Commit/stash first, or set ALLOW_DIRTY=1 to override."
fi
GIT_SHA="$(git rev-parse --short HEAD)"

# Soft login check — credential helpers won't show up here, so only warn.
if [ -f "${HOME}/.docker/config.json" ] && ! grep -q "$REGISTRY" "${HOME}/.docker/config.json"; then
  log "note: no '${REGISTRY}' entry in ~/.docker/config.json — if push 401s, run: docker login ${REGISTRY}"
fi

log "Deploying ${APP}  image=${IMAGE}  tags=[${TAG}, ${GIT_SHA}]  platform=${PLATFORM}"

# --- Build (cross-build for the server's arch) ------------------------------
# buildx is the reliable way to cross-build linux/amd64 on an Apple Silicon
# laptop and load the result into the local daemon. Equivalent to
# `docker build --platform=linux/amd64` where that already uses buildx.
log "Building image for ${PLATFORM} …"
docker buildx build \
  --platform="${PLATFORM}" \
  --pull \
  --load \
  -t "${IMAGE}:${TAG}" \
  -t "${IMAGE}:${GIT_SHA}" \
  .

# --- Push both tags ---------------------------------------------------------
log "Pushing ${IMAGE}:${TAG}"
docker push "${IMAGE}:${TAG}"
log "Pushing ${IMAGE}:${GIT_SHA}"
docker push "${IMAGE}:${GIT_SHA}"

# --- Roll only this service on the server -----------------------------------
# Pull the new :latest for THIS service and recreate just its container; Caddy
# and every other site in the stack are left untouched. On the very FIRST deploy
# the stack does not know this service yet, so compose errors "no such service".
# That is expected, not a failure: the image is already pushed — you now wire the
# service + Caddy block into scaleway-infra and deploy THAT to start it.
log "Rolling '${APP}' on ${HOST} …"
set +e
roll_out="$(ssh "${HOST}" "cd ${STACK_DIR} && docker compose pull ${APP} && docker compose up -d ${APP}" 2>&1)"
roll_rc=$?
set -e
if [ -n "${roll_out}" ]; then printf '%s\n' "${roll_out}"; fi

if [ "${roll_rc}" -ne 0 ]; then
  if printf '%s' "${roll_out}" | grep -qi "no such service"; then
    cat <<EOF

────────────────────────────────────────────────────────────────────────
First-deploy bootstrap: the image is pushed, but the stack has no '${APP}'
service yet, so the roll was skipped (this is expected — not an error).

  Pushed: ${IMAGE}:${TAG}
          ${IMAGE}:${GIT_SHA}

Next:
  1. Add the '${APP}' service block + Caddy block to scaleway-infra.
  2. Run scaleway-infra's deploy.sh — that starts the container (image now
     exists) and reloads Caddy. ${URL} goes live there.
After that, this script alone handles build → push → roll on every deploy.
────────────────────────────────────────────────────────────────────────
EOF
    exit 0
  fi
  die "roll failed on ${HOST} (see compose output above)."
fi

# --- Done -------------------------------------------------------------------
# The container is rolled — but rolling it is NOT the same as serving the URL.
# meridian.discrepancies.eu only resolves once scaleway-infra's Caddy block for
# this app is deployed (that reload is what routes the subdomain to :3000).
log "Rolled '${APP}' @ ${GIT_SHA} on ${HOST}"
printf '\n  Pushed:  %s  (also :%s)\n' "${IMAGE}:${TAG}" "${GIT_SHA}"
printf '  URL:     %s\n' "${URL}"
printf '           serves only after scaleway-infra deploys its Caddy block for %s.\n' "${APP}"
printf '  Verify:  curl -sI %s\n\n' "${URL}"
# Best-effort smoke check — never fails the deploy. No response is normal until
# Caddy is wired for this app, or while first-use TLS issuance is still settling.
curl -sI --max-time 10 "${URL}" || printf '  (no response yet — normal until Caddy routes %s, or while TLS is issuing)\n\n' "${APP}"
