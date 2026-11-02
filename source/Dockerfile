# syntax=docker/dockerfile:1
#
# Swift Download Manager as a download server for a team or an organisation.
#
#   docker run -d --name sdm -p 8080:8080 -v sdm-data:/data \
#     -e SDM_ADMIN_USER=admin -e SDM_ADMIN_PASSWORD='a long password' \
#     ghcr.io/namitjain07/swift-download-manager:latest
#
# Then open http://localhost:8080/ and sign in. Put it behind an HTTPS reverse proxy before it leaves a trusted
# network. Everything (settings, accounts, the download list and the downloaded files) is kept in /data.

FROM rust:1-bookworm AS build
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /data --shell /usr/sbin/nologin sdm \
 && mkdir -p /data/downloads \
 && chown -R sdm:sdm /data
COPY --from=build /src/target/release/swift_download_manager /usr/local/bin/swift-download-manager
USER sdm
ENV SDM_DATA=/data \
    SDM_DOWNLOADS=/data/downloads \
    SDM_BIND=0.0.0.0 \
    SDM_PORT=8080
VOLUME /data
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 CMD ["swift-download-manager", "--healthcheck"]
ENTRYPOINT ["swift-download-manager"]
CMD ["--server"]
