FROM ubuntu@sha256:bcc511d82482900604524a8e8d64bf4c53b2461868dac55f4d04d660e61983cb

RUN apt -yq update && \
    apt -yqq install --no-install-recommends curl ca-certificates \
        build-essential pkg-config libssl-dev llvm-dev liblmdb-dev clang cmake rsync

ENV RUSTUP_HOME=/opt/rustup
ENV CARGO_HOME=/opt/cargo
ENV RUST_VERSION=1.75.0
ENV PATH=${CARGO_HOME}/bin:${PATH}
RUN curl --fail https://sh.rustup.rs -sSf \
        | sh -s -- -y --default-toolchain ${RUST_VERSION}-x86_64-unknown-linux-gnu --no-modify-path && \
    rustup default ${RUST_VERSION}-x86_64-unknown-linux-gnu && \
    rustup target add wasm32-unknown-unknown && \
    cargo install ic-wasm --version 0.7.0 --force && \
    cargo install candid-extractor --version 0.1.2 --force && \
    cargo install just --version 1.24.0 --force

COPY . /cts
WORKDIR /cts

RUN export RUSTFLAGS="--remap-path-prefix ${CARGO_HOME}=/cargo"
RUN just build release