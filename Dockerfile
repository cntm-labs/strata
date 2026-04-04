# Stage 1: Build frontend
FROM node:20-alpine AS frontend
WORKDIR /app/dashboard
COPY dashboard/package*.json ./
RUN npm ci
COPY dashboard/ ./
RUN npm run build

# Stage 2: Build backend
FROM rust:1.83 AS backend
WORKDIR /app
COPY Cargo.toml ./
COPY resource/ ./resource/
RUN cargo build --release

# Stage 3: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=backend /app/target/release/strata .
COPY --from=frontend /app/dashboard/dist ./static/
COPY resource/migrations ./migrations/
EXPOSE 3000
CMD ["./strata"]
