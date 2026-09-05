FROM rust:1.89-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/dodo-be-test /usr/local/bin/invoice-service
COPY --from=build /app/target/release/mock_psp /usr/local/bin/mock-psp
CMD ["/usr/local/bin/invoice-service"]
