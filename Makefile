include build/common.mk

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

web:
	cd web && trunk build --release --features webgl

plugin:
	docker run --rm -v "$(CURDIR)/assets/achievements:/src" mcr.microsoft.com/dotnet/sdk:10.0 sh -c \
		'mkdir /build && cp /src/Plugin.cs /src/BlackforgeAchievements.csproj /build/ && cd /build && dotnet build -c Release -o /out && cp /out/BlackforgeAchievements.dll /src/'
