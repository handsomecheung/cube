FROM rust:1.91.1

RUN rustup target add wasm32-unknown-unknown && \
    cargo install wasm-bindgen-cli
