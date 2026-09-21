# syntax=docker/dockerfile:1.7

# Builds the landing page, then the server with that page embedded in it.

FROM oven/bun:1.4.2 AS bun

FROM rust:trixie AS builder

COPY --from=bun /usr/local/bin/bun /usr/local/bin/bun

ENV CARGO_HOME=/usr/local/cargo
ENV CARGO_TARGET_DIR=/build/target

RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*

RUN cargo install --locked trunk

# The engine repo carries its shared `build` submodule with an ssh url, and
# cargo clones the submodules of a git dependency. A build container has no ssh
# key, so the fetch is rewritten to anonymous https. Both repos are public.
# libgit2 does not read insteadOf rules, so cargo has to shell out to git.
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
RUN git config --global url."https://github.com/".insteadOf "git@github.com:" \
 && git config --global http.version HTTP/1.1

WORKDIR /build

COPY web/package.json web/bun.lock ./web/
RUN cd web && bun install --frozen-lockfile

# The toolchain pin lands first so installing that nightly and its wasm target
# stays a cached layer, separate from the sources.
COPY rust-toolchain.toml ./rust-toolchain.toml
RUN rustup show && rustup target add wasm32-unknown-unknown

COPY . ./
WORKDIR /build/web
RUN bun run check

# webgl is a fallback, browsers with WebGPU still take WebGPU. The page is
# served at the site root, so the public url is /. trunk writes dist/ into the
# crate, outside the cached target dir, so the output survives the mount.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked,id=blackforge-web-cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked,id=blackforge-web-cargo-git \
    --mount=type=cache,target=/build/target,sharing=locked,id=blackforge-web-cargo-target \
    trunk build --release --features webgl --public-url /

# The server embeds web/dist at compile time, so it builds after the page. A
# build with no page in it would serve an empty site, so that stops here.
WORKDIR /build
RUN test -f web/dist/index.html
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked,id=blackforge-web-cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked,id=blackforge-web-cargo-git \
    --mount=type=cache,target=/build/target,sharing=locked,id=blackforge-web-cargo-target \
    cargo build --release --locked -p blackforge-server \
 && cp /build/target/release/blackforge-server /tmp/blackforge-server

FROM debian:trixie-slim

# The server calls Google and studio over TLS and checks them against the
# system roots.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /tmp/blackforge-server /usr/local/bin/blackforge-server

EXPOSE 3000
CMD ["blackforge-server"]
