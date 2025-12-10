# Build Stage
FROM rust:alpine AS builder

WORKDIR /app

# Install build dependencies (musl-dev is essential for Alpine/Rust)
RUN apk add --no-cache musl-dev

# 1. Create a dummy project to cache dependencies
RUN mkdir -p src
RUN echo "fn main() {}" > src/main.rs
COPY Cargo.toml Cargo.lock ./

# Build dependencies only
RUN cargo build --release

# 2. Build the actual application
COPY src ./src
# Touch main.rs to force rebuild of the app itself (since we changed it from dummy)
RUN touch src/main.rs
RUN cargo build --release

# Runtime Stage
FROM alpine:latest

WORKDIR /app

# Install runtime dependencies if any (e.g. ca-certificates for HTTPS)
RUN apk add --no-cache ca-certificates tzdata

# Copy binary from builder
COPY --from=builder /app/target/release/space-api-rs .

# Create a non-root user (optional but recommended, skipping for simplicity unless requested)

# Expose Rocket default port
EXPOSE 8000

# Set environment variable for config path (default to /app/config.toml)
ENV CONFIG_PATH=/app/config.toml
ENV ROCKET_ADDRESS=0.0.0.0

CMD ["./space-api-rs"]
