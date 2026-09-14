# syntax=docker/dockerfile:1
# Pinned by digest, the same builder and runtime images the sibling project trusts
# (research.md, Docker official Rust and Debian images).
#
# The engine reaches the sibling rules core by path (../../battle-snake-rust from engine/),
# so the sibling is supplied as a named build context and only its manifest and sources are
# copied; build it with:
#   docker build --build-context sibling=../battle-snake-rust -t tiger-engine .
FROM rust:1.98.1-slim-bookworm@sha256:ebd900bae66fd508b466cef82d64a83a5fb34682e4c8b2797a42908bddc95a57 AS builder
WORKDIR /build
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY engine ./engine
COPY sparring ./sparring
COPY --from=sibling Cargo.toml rust-toolchain.toml /battle-snake-rust/
COPY --from=sibling src /battle-snake-rust/src
RUN cargo build --release --locked -p tiger-engine

FROM debian:bookworm-20260824-slim@sha256:88200866dfff7ea7f5cbcb6ec7c8a701889efe6fe859fe64d6990e4b07ea4171 AS runtime
RUN useradd --system --no-create-home --uid 10001 battlesnake
COPY --from=builder /build/target/release/tiger-engine /usr/local/bin/tiger-engine
USER battlesnake
ENV BIND_ADDR=0.0.0.0 \
    PORT=8080
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/tiger-engine"]
