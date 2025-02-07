
# Build stage
FROM rust:latest as builder

WORKDIR /app
# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy the Cargo.toml and Cargo.lock files
COPY Cargo.* ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs

# Build dependencies (this will be cached)
RUN cargo build --release

# Remove the dummy source
RUN rm -rf src

# Copy the actual source code
COPY src ./src

# Build the actual application
RUN cargo build --release

# Runtime stage
FROM --platform=$TARGETPLATFORM gcr.io/distroless/cc-debian12

WORKDIR /app

# Copy the built binary from the builder stage
COPY --from=builder /app/target/release/geranium /app/geranium

# Set the binary as the entrypoint
ENTRYPOINT ["/app/geranium"]
