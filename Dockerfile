FROM rust:latest AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations

RUN apt-get update && apt-get install -y --no-install-recommends libsqlite3-dev && rm -rf /var/lib/apt/lists/*
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libsqlite3-0 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/projects-service /usr/local/bin/projects-service
COPY --from=builder /app/migrations ./migrations
RUN groupadd -g 999 appgroup && useradd -r -u 999 -g 999 -s /bin/false appuser && mkdir -p /data && chown -R 999:999 /data
USER appuser
EXPOSE 3001
CMD ["projects-service"]
