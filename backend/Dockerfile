FROM rust:1-bookworm AS development

RUN apt-get -o Acquire::Retries=3 update \
  && apt-get install -y --no-install-recommends curl ffmpeg \
  && rm -rf /var/lib/apt/lists/* \
  && rustup component add clippy rustfmt

WORKDIR /app
EXPOSE 8080
CMD ["cargo", "run", "-p", "novex-api", "--bin", "novex-api"]

FROM rust:1-bookworm AS build

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY backend ./backend
COPY crates ./crates
COPY agent-definitions ./agent-definitions
RUN cargo build --release -p novex-api --bin novex-api

FROM debian:bookworm-slim AS runtime

RUN apt-get -o Acquire::Retries=3 update \
  && apt-get install -y --no-install-recommends ca-certificates curl ffmpeg \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /app
ENV NOVEX_AGENT_DEFINITIONS_DIR=/app/agent-definitions
COPY --from=build /build/target/release/novex-api /usr/local/bin/novex-api
COPY --from=build /build/agent-definitions ./agent-definitions
EXPOSE 8080
CMD ["novex-api"]
