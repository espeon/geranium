
# Build stage
FROM rust:latest as builder

WORKDIR /app
# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid 6969 \
    app

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
FROM gcr.io/distroless/cc

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /app

# Copy the built binary from the builder stage
COPY --from=builder /app/target/release/geranium /app/geranium

USER app:app

EXPOSE 3000

# Set the binary as the entrypoint
ENTRYPOINT ["/app/geranium"]
