# Multi-stage build for Record service
FROM rust:1.87.0-bookworm AS builder

# Install system dependencies required for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    libgstreamer-plugins-bad1.0-dev \
    libssl-dev \
    ca-certificates \
    git \
    curl \
    libnice-dev \
    gstreamer1.0-nice \
    gstreamer1.0-plugins-bad \
    && rm -rf /var/lib/apt/lists/*

# Update ca-certificates and configure cargo/git for SSL
RUN update-ca-certificates
RUN git config --global http.sslVerify true

# Set working directory
WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy src directory to build dependencies first
RUN mkdir src && \
    echo "fn main() {println!(\"If you see this, the build broke\")}" > src/main.rs

# Configure cargo to use git for dependencies that might have issues
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
ENV CARGO_HTTP_CAINFO=/etc/ssl/certs/ca-certificates.crt
ENV CARGO_HTTP_CHECK_REVOKE=false

# Create cargo config to disable SSL check for build environment
RUN mkdir -p ~/.cargo && \
    echo '[http]' > ~/.cargo/config.toml && \
    echo 'check-revoke = false' >> ~/.cargo/config.toml

# Temporarily disable SSL verification for build (not recommended for production)
RUN git config --global http.sslVerify false

# Build dependencies only
RUN cargo build --release && \
    rm -f target/release/deps/record_service*

# Copy actual source code
COPY src ./src
COPY migrations ./migrations

# Build the actual application
COPY .sqlx ./.sqlx
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    gstreamer1.0-tools \
    gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-libav \
    gstreamer1.0-plugins-ugly \
    netcat-openbsd \
    libnice10 \
    gstreamer1.0-nice \
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