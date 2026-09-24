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

# The game plugins compile against the game, VALHEIM_MANAGED is the
# valheim_Data/Managed folder of an installed Valheim. Both take
# assets/shared, so the build copies it next to the project.
join-plugin:
	docker run --rm -v "$(CURDIR)/assets:/src" -v "$(VALHEIM_MANAGED):/managed:ro" mcr.microsoft.com/dotnet/sdk:10.0 sh -c \
		'mkdir -p /build/join && cp -r /src/shared /build/ && cp /src/join/*.cs /src/join/BlackforgeJoin.csproj /build/join/ && cd /build/join && dotnet build -c Release -o /out -p:ValheimManaged=/managed && cp /out/BlackforgeJoin.dll /src/join/'

status-plugin:
	docker run --rm -v "$(CURDIR)/assets:/src" -v "$(VALHEIM_MANAGED):/managed:ro" mcr.microsoft.com/dotnet/sdk:10.0 sh -c \
		'mkdir -p /build/status && cp -r /src/shared /build/ && cp /src/status/*.cs /src/status/BlackforgeStatus.csproj /build/status/ && cd /build/status && dotnet build -c Release -o /out -p:ValheimManaged=/managed && cp /out/BlackforgeStatus.dll /src/status/'
