# syntax=docker/dockerfile:1

FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release

FROM alpine:3.21 AS runner

RUN apk add --no-cache ca-certificates tzdata

RUN addgroup -S appgroup && adduser -S appuser -G appgroup

WORKDIR /app

COPY --from=builder /app/target/release/apple-map-proxy /app/apple-map-proxy

USER appuser:appgroup

EXPOSE 8080

ENV HOST=0.0.0.0 \
    PORT=8080

ENTRYPOINT ["/app/apple-map-proxy"]
