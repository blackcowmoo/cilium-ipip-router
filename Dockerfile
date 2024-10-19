FROM rust:1.82.0 as builder
WORKDIR /usr/src/router

RUN cargo init .
COPY Cargo* ./
RUN cargo build --release && rm target/release/deps/project*

COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
COPY --from=builder /usr/src/project/target/release/router /usr/local/bin/router
CMD ["router"]
