ARG BUILD_FROM

# =============================================================================
# Stage 1: Compile the voip-engine Rust binary
#
# Source is cloned from the repository at build time — no manual file copying
# needed. BUILD_VERSION (injected by HA supervisor) busts the Docker cache
# on every version bump so you always get fresh code.
# =============================================================================
FROM rust:1.85-bookworm AS builder

ARG BUILD_VERSION=dev

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        cmake \
        pkg-config \
        libssl-dev \
        protobuf-compiler \
        libprotobuf-dev \
        git \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Clone the repository (cache is busted by BUILD_VERSION on each release)
RUN echo "Building version: ${BUILD_VERSION}" \
    && git clone --depth 1 https://github.com/nitish-mp3/simson.git /repo \
    && cp -r /repo/voip-engine/. /build/ \
    && rm -rf /repo

# Build the release binary
RUN cargo build --release \
    && strip ./target/release/voip-engine

# =============================================================================
# Stage 2: HA add-on runtime image
# =============================================================================
FROM ${BUILD_FROM}

# Runtime libraries
RUN apt-get update && apt-get install -y --no-install-recommends \
        libssl3 \
        ca-certificates \
        curl \
        jq \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get purge -y --auto-remove

# Install compiled binary
COPY --from=builder /build/target/release/voip-engine /usr/local/bin/voip-engine
RUN chmod 755 /usr/local/bin/voip-engine

# Data directories (add-on data dir /data is auto-mounted by HA)
RUN mkdir -p /data/voip/recordings /data/voip/certs

# Entry script
COPY run.sh /run.sh
RUN chmod a+x /run.sh

CMD ["/run.sh"]
