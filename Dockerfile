# syntax=docker/dockerfile:1.7

# stage-1: builder
FROM rust:1-slim AS builder

WORKDIR /app

# Cache deps separately from sources. The dummy `benches/convert.rs` is
# required because Cargo.toml declares the bench target; without it the
# manifest fails to parse during the deps-only build.
COPY Cargo.toml Cargo.lock ./
RUN mkdir ./src ./benches && \
    echo 'fn main() {}' > ./src/main.rs && \
    echo '' > ./src/lib.rs && \
    echo 'fn main() {}' > ./benches/convert.rs && \
    cargo build --release && \
    rm -rf ./src ./benches

# Build the real binary.
COPY ./src ./src
COPY ./benches ./benches
RUN touch -a -m ./src/main.rs ./src/lib.rs ./benches/convert.rs && \
    cargo build --release

# stage-2: minimal runtime image with just the binary.
FROM debian:bookworm-slim

WORKDIR /app
COPY --from=builder /app/target/release/exportbranch /usr/local/bin/exportbranch

ENTRYPOINT ["exportbranch"]
