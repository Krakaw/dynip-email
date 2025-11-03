# Multi-stage build for dynip-email
FROM rust:1.90-slim AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy Cargo files first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies (this layer will be cached unless Cargo.toml changes)
RUN cargo build --release && rm -rf src

# Copy source code
COPY src ./src
COPY static ./static

# Build the actual application
RUN cargo build --release

# Runtime stage
FROM debian:trixie-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsqlite3-0 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /bin/false dynip-email

# Create app directory
WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/dynip-email /app/dynip-email

# Copy static files
COPY --from=builder /app/static /app/static

# Create data directory for database
RUN mkdir -p /app/data \
    && chmod 755 /app/data \
    && chown -R dynip-email:dynip-email /app/data

# Switch to non-root user
USER dynip-email

# Expose ports
EXPOSE 3000 2525 587 465

# Set environment variables
ENV RUST_LOG=info
ENV SMTP_PORT=2525
ENV API_PORT=3000
ENV DATABASE_URL=sqlite:/app/data/emails.db
ENV DOMAIN_NAME=tempmail.local

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/api/emails/health@test.com || exit 1

# Run the application
CMD ["./dynip-email"]