FROM rust:1-bookworm

WORKDIR /app

RUN apt-get update \
  && apt-get install -y --no-install-recommends curl \
  && rm -rf /var/lib/apt/lists/*

RUN rustup component add clippy

EXPOSE 8080

CMD ["cargo", "run", "--bin", "novex-api"]
