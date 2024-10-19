FROM rust:1.82.0 as builder
WORKDIR /usr/src/router

COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
COPY --from=builder /usr/src/router/target/release/router /usr/local/bin/router
CMD ["router"]
