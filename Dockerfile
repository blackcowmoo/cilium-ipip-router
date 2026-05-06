FROM rust:1.95.0 as builder
WORKDIR /usr/src/router

COPY . .

RUN apt-get update && apt-get install -y --no-install-recommends musl-tools && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends iproute2 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/router/target/x86_64-unknown-linux-musl/release/router /usr/local/bin/router
COPY resources/log4rs.yaml /var/lib/router/resources/log4rs.yaml
WORKDIR /var/lib/router
CMD ["router"]
