FROM rust:1.90-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates sqlite3 \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/projects-service /usr/local/bin/projects-service
COPY --from=builder /app/migrations ./migrations

ENV HOST=0.0.0.0
ENV DATABASE_URL=sqlite:///data/projects.db

EXPOSE 3001

CMD ["sh", "-c", "mkdir -p /data && touch /data/projects.db && projects-service"]
