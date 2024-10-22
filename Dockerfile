FROM rust:1.82.0 as builder
WORKDIR /usr/src/router

COPY . .
RUN cargo build --release --target x86_64-unknown-linux-gnu

FROM debian:bullseye-slim
COPY --from=builder /usr/src/router/target/x86_64-unknown-linux-gnu/release/router /usr/local/bin/router
CMD ["router"]
