# Portable image for any Docker host (PC, Raspberry Pi, NAS).
# On an Android phone, use the native Termux build instead
# (scripts/install-termux.sh) — Docker is not available there.
#
#   docker build -t glasspane .
#   docker run -d -p 8090:8090 -e HA_TOKEN=... \
#     -v $PWD/glasspane.toml:/app/glasspane.toml \
#     -v $PWD/data:/data -v $PWD/photos:/photos glasspane

# 1. Frontend: TypeScript + Preact → one Chrome-57-safe bundle.
FROM node:22-slim AS web
WORKDIR /web
COPY web/package*.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# 2. Server: a single static-ish Rust binary.
FROM rust:1-slim AS server
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --bin glasspane

# 3. Runtime: binary + built frontend. curl is used for the AQI lookup.
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends curl ca-certificates \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=server /app/target/release/glasspane /usr/local/bin/glasspane
COPY --from=web /web/dist ./web/dist
COPY glasspane.example.toml ./glasspane.toml
EXPOSE 8090
CMD ["glasspane", "glasspane.toml"]
