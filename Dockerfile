# syntax=docker/dockerfile:1
#
# Meridian — Leptos (SSR + WASM hydrate) served by Axum, listening on :3000.
# Always build for linux/amd64 (the Scaleway server) — see deploy.sh.

##############################################################################
# Stage 1 — build the Rust server binary + the client site bundle.
##############################################################################
FROM rust:1-bookworm AS app-builder

# cargo-leptos drives the dual (server + wasm) build; the wasm half needs the
# bare-metal target. Pull a prebuilt cargo-leptos via binstall instead of
# compiling it from source (which is painfully slow under amd64 emulation).
RUN rustup target add wasm32-unknown-unknown \
 && curl -L --proto '=https' --tlsv1.2 -sSf \
      https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash \
 && cargo binstall -y cargo-leptos

WORKDIR /build
COPY . .

# Release build → server bin at target/release/meridian, site at target/site.
# (Style is plain CSS, so no sass toolchain is needed.)
RUN cargo leptos build --release -vv

##############################################################################
# Stage 2 — bake the SQLite cache the app reads at request time.
# seed_demo.py is fully OFFLINE. For live data, swap this stage's command for
# the real pipeline (fetch_filings.py + parse_xbrl_json.py [+ fetch_fx_rates.py]),
# which needs network — or mount a host DB instead (see deploy.sh notes).
##############################################################################
FROM ghcr.io/astral-sh/uv:python3.11-bookworm-slim AS data-builder
WORKDIR /build
COPY pipeline/ ./pipeline/
# db.py resolves the repo root three parents above pipeline/src, so the cache
# lands at /build/data/meridian.db.
RUN mkdir -p data \
 && cd pipeline \
 && uv sync --locked \
 && uv run python src/seed_demo.py

##############################################################################
# Stage 3 — minimal runtime image.
##############################################################################
FROM debian:bookworm-slim AS runtime

# rusqlite is built with the "bundled" feature, so no system SQLite is required.
# Run unprivileged; give the app a writable data dir (SQLite may need to touch a
# lock/journal beside the DB even for reads).
RUN useradd --system --uid 10001 --home-dir /app --shell /usr/sbin/nologin meridian \
 && install -d -o meridian -g meridian /app/data
WORKDIR /app

COPY --from=app-builder  /build/target/release/meridian  /app/meridian
COPY --from=app-builder  /build/target/site              /app/site
COPY --from=data-builder --chown=meridian:meridian \
     /build/data/meridian.db  /app/data/meridian.db

# main.rs reads config from the environment when LEPTOS_OUTPUT_NAME is set (it
# has no Cargo.toml to fall back to here). Bind 0.0.0.0 so Caddy can reach it.
ENV LEPTOS_OUTPUT_NAME=meridian \
    LEPTOS_SITE_ROOT=/app/site \
    LEPTOS_SITE_PKG_DIR=pkg \
    LEPTOS_SITE_ADDR=0.0.0.0:3000 \
    LEPTOS_ENV=PROD \
    MERIDIAN_DB=/app/data/meridian.db \
    RUST_LOG=info

USER meridian
EXPOSE 3000
CMD ["/app/meridian"]
