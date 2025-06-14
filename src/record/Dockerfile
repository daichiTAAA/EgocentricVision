# Multi-stage build for Record service
FROM rust:1.82-bookworm AS builder

# Install system dependencies required for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy src directory to build dependencies first
RUN mkdir src && \
    echo "fn main() {println!(\"If you see this, the build broke\")}" > src/main.rs && \
    cargo build --release && \
    rm -f target/release/deps/record_service*

# Copy actual source code
COPY src ./src
COPY migrations ./migrations

# Build the actual application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false appuser

# Create directories
RUN mkdir -p /app /var/data/recordings /app/config /app/migrations
RUN chown -R appuser:appuser /app /var/data/recordings

# Copy binary from builder stage
COPY --from=builder /app/target/release/record-service /app/record-service
COPY --chown=appuser:appuser migrations /app/migrations

# Make binary executable
RUN chmod +x /app/record-service

# Switch to app user
USER appuser

# Set working directory
WORKDIR /app

# Expose port
EXPOSE 3000

# Run the application
CMD ["./record-service"]