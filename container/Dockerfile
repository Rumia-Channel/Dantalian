FROM rust:1.96-bookworm AS builder

WORKDIR /src
COPY . .
RUN cargo build --release --bin audio_processor --features native,audio-codec

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 dantalian \
    && useradd --system --uid 10001 --gid dantalian --home-dir /nonexistent --shell /usr/sbin/nologin dantalian

COPY --from=builder /src/target/release/audio_processor /usr/local/bin/audio_processor

USER dantalian
ENTRYPOINT ["/usr/local/bin/audio_processor"]
