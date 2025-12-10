# Build Stage
FROM rust:alpine AS builder

WORKDIR /app

# Install build dependencies (musl-dev is essential for Alpine/Rust)
# Add nasm so rav1e can build its ASM
RUN apk add --no-cache musl-dev nasm

# 1. Create a dummy project to cache dependencies
RUN mkdir -p src
RUN echo "fn main() {}" > src/main.rs
COPY Cargo.toml Cargo.lock ./

# Build dependencies only
RUN cargo build --release

# 2. Build the actual application
COPY src ./src
RUN touch src/main.rs
RUN cargo build --release

# Runtime Stage
FROM alpine:latest

WORKDIR /app

RUN apk add --no-cache ca-certificates tzdata

# Copy binary from builder
COPY --from=builder /app/target/release/space-api-rs .

EXPOSE 3000

ENV CONFIG_PATH=/app/config.toml
ENV ROCKET_ADDRESS=0.0.0.0

CMD ["./space-api-rs"]
