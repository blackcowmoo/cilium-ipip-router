FROM rust:1.67 as builder
WORKDIR /usr/src/router

COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
COPY --from=builder /usr/src/project/target/release/router /usr/local/bin/router
CMD ["router"]
