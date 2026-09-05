$ErrorActionPreference = "Stop"

docker compose up -d --build
# docker compose up -d invoice-service --build
cargo test
