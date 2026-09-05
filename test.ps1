$ErrorActionPreference = "Stop"

docker compose up -d --build
cargo test
