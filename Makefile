include build/common.mk

# Build and run the release binary. It uses the Sentry DSN from Infisical when
# it is logged in, and falls back to a plain build otherwise. The logic is in
# the shared run.sh.
INFISICAL_PROJECT := a2066daf-13f4-4831-9d06-8ee963560c03

run:
	INFISICAL_PROJECT=$(INFISICAL_PROJECT) PATH="$$HOME/.cargo/bin:$$PATH" sh build/run.sh

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

# The join plugin compiles against the game, VALHEIM_MANAGED is the
# valheim_Data/Managed folder of an installed Valheim.
join-plugin:
	docker run --rm -v "$(CURDIR)/assets/join:/src" -v "$(VALHEIM_MANAGED):/managed:ro" mcr.microsoft.com/dotnet/sdk:10.0 sh -c \
		'mkdir /build && cp /src/Plugin.cs /src/BlackforgeJoin.csproj /build/ && cd /build && dotnet build -c Release -o /out -p:ValheimManaged=/managed && cp /out/BlackforgeJoin.dll /src/'
