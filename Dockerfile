# syntax=docker/dockerfile:1.6
#
# Multi-stage Dockerfile for image-rs.
#
# Stages:
#   * web-builder: Node 20 compiles the Vue 3 frontend (`web/dist`).
#   * rust-builder: Rust 1.85 + libopencv-dev (4.6) compiles the backend.
#   * runtime: Debian Bookworm slim + opencv 4.6 runtime shared libs + fonts.
#
# BuildKit cache mounts make incremental rebuilds fast.
#
# Format support: JPEG / PNG / WebP. AVIF is NOT available — Debian Bookworm
# (and all current mainstream distros except Arch) ship libopencv built
# without WITH_AVIF. To add AVIF, switch the runtime to archlinux:base
# (`pacman -S opencv libavif libwebp`) or compile OpenCV 4.11+ from source
# in the builder stage with `-DWITH_AVIF=ON -DWITH_WEBP=ON`.

# ─── web frontend builder ────────────────────────────────────────────────────
FROM node:20-bookworm AS web-builder

WORKDIR /web
COPY web/package.json web/package-lock.json* ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci
COPY web/ ./
RUN npm run build
# Output is /web/dist/

# ─── rust backend builder ────────────────────────────────────────────────────
FROM rust:1.85-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        libopencv-dev \
        clang \
        libclang-dev \
        pkg-config \
        cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY src ./src

# Cache cargo registry + target across builds via BuildKit cache mounts.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release --bin image-rs && \
    cp target/release/image-rs /tmp/image-rs

# ─── runtime ─────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# Install only the OpenCV shared libs we actually link against, plus a CJK
# fallback font and ca-certificates. Adjust the libopencv-* package names if
# building on a distro that ships different versioned package suffixes.
RUN apt-get update && apt-get install -y --no-install-recommends \
        libopencv-core406 \
        libopencv-imgcodecs406 \
        libopencv-imgproc406 \
        fonts-dejavu-core \
        fonts-noto-cjk \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home --shell /sbin/nologin image-rs

WORKDIR /app
COPY --from=builder /tmp/image-rs /usr/local/bin/image-rs
COPY --from=web-builder /web/dist /app/web/dist

ENV IMAGE_RS_BIND=0.0.0.0:8080 \
    IMAGE_RS_LOG=info \
    IMAGE_RS_MAX_UPLOAD=20971520 \
    IMAGE_RS_REQUEST_TIMEOUT_SECS=30 \
    IMAGE_RS_FONT_DIR=/usr/share/fonts/truetype/dejavu \
    IMAGE_RS_UI_DIR=/app/web/dist

USER image-rs
EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/image-rs"]
