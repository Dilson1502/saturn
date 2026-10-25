FROM rust:1.88-slim-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --locked --release --bin main

FROM debian:bookworm-slim

WORKDIR /app
COPY --from=builder /app/target/release/main /usr/local/bin/main
COPY static ./static
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

ENV API_URL="https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_month.geojson"

USER 10001:10001
EXPOSE 5005
CMD ["main"]
