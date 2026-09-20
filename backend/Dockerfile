# syntax=docker/dockerfile:1.7

# Builds the landing page from the workspace and serves the static output.

FROM oven/bun:1.4.2 AS bun

FROM rust:trixie AS wasm

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

FROM nginx:alpine

COPY --from=wasm /build/web/dist /usr/share/nginx/html
COPY backend/nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
