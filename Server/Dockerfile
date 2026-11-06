FROM rust:1.97.1-trixie AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --locked --release

FROM debian:trixie-slim AS runtime

COPY --from=builder /build/target/release/setochka /usr/local/bin/setochka

ENTRYPOINT ["/usr/local/bin/setochka"]
