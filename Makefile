check:
	typos
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo machete
	cargo test --workspace
	cd web && bun install --frozen-lockfile && bun run check && bun run build

fix:
	cargo fmt --all
	cargo clippy --fix --allow-dirty --allow-staged --workspace --all-targets

release-mac:
	cargo run --locked --manifest-path build/Cargo.toml --bin release-mac

release-linux:
	cargo run --locked --manifest-path build/Cargo.toml --bin release-linux

release-win:
	cargo run --locked --manifest-path build/Cargo.toml --bin release-win

manifest:
	cargo run --locked --manifest-path build/Cargo.toml --bin release-manifest

web:
	cd web && trunk build --release --features webgl
