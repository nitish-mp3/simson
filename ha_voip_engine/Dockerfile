ARG BUILD_FROM

# =============================================================================
# Stage 1: Compile the voip-engine Rust binary
# Source is cloned from GitHub at build time. BUILD_VERSION (injected by HA
# supervisor) busts the Docker layer cache on every version bump.
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

RUN echo "Building version: ${BUILD_VERSION}" \
    && git clone --depth 1 https://github.com/nitish-mp3/simson.git /repo \
    && cp -r /repo/voip-engine/. /build/ \
    && rm -rf /repo

RUN cargo build --release \
    && strip ./target/release/voip-engine

# =============================================================================
# Stage 2: HA add-on runtime image
# =============================================================================
FROM ${BUILD_FROM}

RUN apt-get update && apt-get install -y --no-install-recommends \
        libssl3 \
        ca-certificates \
        curl \
        jq \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get purge -y --auto-remove

COPY --from=builder /build/target/release/voip-engine /usr/local/bin/voip-engine
RUN chmod 755 /usr/local/bin/voip-engine

RUN mkdir -p /data/voip/recordings /data/voip/certs

COPY run.sh /run.sh
RUN chmod a+x /run.sh

CMD ["/run.sh"]
