# syntax=docker/dockerfile:1.7

# stage-1: builder
FROM rust:1-slim AS builder

WORKDIR /app

# Cache deps separately from sources.
COPY Cargo.toml Cargo.lock ./
RUN mkdir ./src && \
    echo 'fn main() {}' > ./src/main.rs && \
    echo '' > ./src/lib.rs && \
    cargo build --release && \
    rm -rf ./src

# Build the real binary.
COPY ./src ./src
RUN touch -a -m ./src/main.rs ./src/lib.rs && \
    cargo build --release

# stage-2: minimal runtime image with just the binary.
FROM debian:bookworm-slim

WORKDIR /app
COPY --from=builder /app/target/release/exportbranch /usr/local/bin/exportbranch

ENTRYPOINT ["exportbranch"]
