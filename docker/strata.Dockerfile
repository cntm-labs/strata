# Multi-stage Dockerfile for Strata (Rust + Vue)
# Stage 1: Build Frontend (Vue 3)
FROM node:20-alpine AS frontend-builder
WORKDIR /app/dashboard
COPY dashboard/package*.json ./
RUN npm install
COPY dashboard/ ./
RUN npm run build

# Stage 2: Build Backend (Rust)
FROM rust:1.85-alpine AS backend-builder
RUN apk add --no-cache musl-dev openssl-dev pkgconfig
WORKDIR /app
COPY resource/ ./resource/
COPY Cargo.toml Cargo.lock ./
# Note: Building only the resource package if it is a workspace
RUN cargo build --release -p strata-resource

# Stage 3: Final Production Image
FROM alpine:3.19
RUN apk add --no-cache libgcc openssl
WORKDIR /app
COPY --from=backend-builder /app/target/release/strata-server ./strata-server
COPY --from=frontend-builder /app/dashboard/dist ./static
# Ensure healthcheck works
RUN apk add --no-cache curl
EXPOSE 3000
CMD ["./strata-server"]
