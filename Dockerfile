FROM rust:latest AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates sqlite3 \
  && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false appuser && \
    mkdir -p /home/appuser/data && \
    chown appuser /home/appuser/data

WORKDIR /app

COPY --from=builder /app/target/release/projects-service /usr/local/bin/projects-service
COPY --from=builder /app/migrations ./migrations

ENV HOST=0.0.0.0
ENV DATABASE_URL=sqlite:////home/appuser/data/projects.db

USER appuser
EXPOSE 8080

CMD ["/usr/local/bin/projects-service"]
