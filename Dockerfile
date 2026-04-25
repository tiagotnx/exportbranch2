# syntax=docker/dockerfile:1.7

# stage-1: builder
FROM rust:1-slim AS builder

WORKDIR /app

# Cache deps separately from sources. Dummy stubs are needed for every
# target Cargo.toml declares, otherwise the manifest fails to parse
# during the deps-only build. `build.rs` is needed because the binary
# pulls env vars set there into `--version`. `.cargo/config.toml` pins
# the safe target-cpu (Ivy Bridge baseline) and must be present before
# the first build.
COPY Cargo.toml Cargo.lock ./
COPY .cargo ./.cargo
RUN mkdir ./src ./benches && \
    echo 'fn main() {}' > ./src/main.rs && \
    echo '' > ./src/lib.rs && \
    echo 'fn main() {}' > ./benches/convert.rs && \
    echo 'fn main() {}' > ./benches/walk.rs && \
    printf 'fn main() {\n    println!("cargo:rustc-env=GIT_SHA=unknown");\n    println!("cargo:rustc-env=GIT_DATE=unknown");\n}\n' > ./build.rs && \
    cargo build --release && \
    rm -rf ./src ./benches ./build.rs

# Build the real binary.
COPY ./src ./src
COPY ./benches ./benches
COPY ./build.rs ./build.rs
RUN touch -a -m \
        ./src/main.rs ./src/lib.rs \
        ./benches/convert.rs ./benches/walk.rs \
        ./build.rs && \
    cargo build --release

# stage-2: minimal runtime image with just the binary.
FROM debian:bookworm-slim

WORKDIR /app
COPY --from=builder /app/target/release/exportbranch /usr/local/bin/exportbranch

ENTRYPOINT ["exportbranch"]
