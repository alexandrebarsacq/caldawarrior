# Builder: rust:1.85-bookworm pinned 2026-03-01 (1.85 = minimum for Cargo edition 2024).
# Runner:  archlinux:base (rolling) — used for taskwarrior 3.x; Debian only ships 2.6.x.
# To upgrade Rust: update the builder FROM tag below.

# Stage 1 — builder (rust:1.85-bookworm pinned 2026-03-01; 1.85 is minimum for Cargo edition 2024)
FROM rust:1.85-bookworm AS builder

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release && \
    cp /build/target/release/caldawarrior /build/caldawarrior

# Stage 2 — runner (archlinux:base for up-to-date taskwarrior 3.x; Debian only ships 2.6.x)
# archlinux:base is rolling — rebuild periodically to pick up taskwarrior updates
FROM archlinux:base

RUN pacman -Syu --noconfirm && \
    pacman -S --noconfirm python python-pip curl gnupg task && \
    pacman -Scc --noconfirm

# pip freeze (pinned for auditability): robotframework==7.0.1 icalendar==5.0.12 requests==2.31.0
# --break-system-packages safe in Docker containers (PEP 668)
RUN pip install --no-cache-dir --break-system-packages \
    robotframework==7.0.1 \
    icalendar==5.0.12 \
    requests==2.31.0

COPY --from=builder /build/caldawarrior /usr/local/bin/caldawarrior

RUN useradd -m testrunner

USER testrunner

WORKDIR /tests
