ARG BUILD_FROM

# =============================================================================
# Stage 1: Compile the voip-engine Rust binary
#
# Run `bash prepare_addon.sh` from the repo root first to copy the Rust source
# into ha_voip_engine/voip-engine/ before building this image.
# =============================================================================
FROM rust:1.75-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        cmake \
        pkg-config \
        libssl-dev \
        protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependency compilation: copy manifests first, build dummy, then full source
COPY voip-engine/Cargo.toml voip-engine/Cargo.lock* ./
RUN mkdir -p src && echo 'fn main(){}' > src/main.rs \
    && cargo build --release 2>/dev/null || true \
    && rm -rf src

# Copy actual source and build
COPY voip-engine/src/ ./src/
COPY voip-engine/build.rs ./
COPY voip-engine/proto/ ./proto/

RUN touch src/main.rs \
    && cargo build --release \
    && strip ./target/release/voip-engine

# =============================================================================
# Stage 2: HA add-on runtime image
# =============================================================================
FROM ${BUILD_FROM}

# Install only the runtime libraries the binary needs
RUN apt-get update && apt-get install -y --no-install-recommends \
        libssl3 \
        ca-certificates \
        curl \
        jq \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get purge -y --auto-remove

# Copy compiled binary
COPY --from=builder /build/target/release/voip-engine /usr/local/bin/voip-engine
RUN chmod 755 /usr/local/bin/voip-engine

# Create data directories
RUN mkdir -p /data/voip/recordings /data/voip/certs

# Entry script
COPY run.sh /run.sh
RUN chmod a+x /run.sh

CMD ["/run.sh"]
