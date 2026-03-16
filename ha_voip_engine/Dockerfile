ARG BUILD_FROM
ARG BUILD_ARCH=amd64
ARG BUILD_VERSION=dev

# =============================================================================
# Single-stage HA add-on image.
#
# The voip-engine binary is pre-built for all 4 architectures by GitHub
# Actions and attached to each GitHub Release.  HA supervisor injects
# BUILD_VERSION (= config.yaml version) and BUILD_ARCH automatically so
# the correct binary is downloaded.  Build time: ~20 seconds instead of
# 30+ minutes.
# =============================================================================
FROM ${BUILD_FROM}

ARG BUILD_ARCH
ARG BUILD_VERSION

RUN apt-get update && apt-get install -y --no-install-recommends \
        libssl3 \
        ca-certificates \
        curl \
        jq \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get purge -y --auto-remove

# Download the pre-built binary for this architecture from GitHub Releases.
RUN curl -fSL \
    "https://github.com/nitish-mp3/simson/releases/download/${BUILD_VERSION}/voip-engine-${BUILD_ARCH}" \
    -o /usr/local/bin/voip-engine \
    && chmod 755 /usr/local/bin/voip-engine

RUN mkdir -p /data/voip/recordings /data/voip/certs

COPY run.sh /run.sh
RUN chmod a+x /run.sh

CMD ["/run.sh"]
