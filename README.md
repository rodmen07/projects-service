# projects-service

Rust / Axum microservice managing client portal data — projects, milestones, deliverables, and messages. Deployed on Fly.io as part of the Portfolio v1.0 Client Portal release.

## Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/health` | none | Liveness check |
| GET | `/api/projects` | JWT | List projects (admin: all; client: own) |
| POST | `/api/projects` | JWT (admin) | Create project |
| GET | `/api/projects/:id/milestones` | JWT | List milestones |
| POST | `/api/projects/:id/milestones` | JWT (admin) | Create milestone |
| GET | `/api/projects/:id/deliverables` | JWT | List deliverables |
| POST | `/api/projects/:id/deliverables` | JWT (admin) | Create deliverable |
| GET | `/api/projects/:id/messages` | JWT | List messages |
| POST | `/api/projects/:id/messages` | JWT | Post message |

## Auth

JWT (HS256) validated against `AUTH_JWT_SECRET`. Role-based: `admin` sees all projects; `client` sees only projects where `client_subject` matches their JWT `sub` claim. `AUTH_ENFORCED=true` by default.

## Tech

Rust · Axum 0.8 · SQLx 0.8 · SQLite · jsonwebtoken 9 · Tower rate limiting · Fly.io

## Running locally

```bash
cp .env.example .env
# edit AUTH_JWT_SECRET to match your auth-service
cargo run
```

## Deployment

```bash
fly deploy --app projects-service-rodmen07
```

Persistent SQLite stored on a Fly volume (`projects_data` → `/data/projects.db`).
