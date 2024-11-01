FROM rust:1.82.0-slim-bullseye AS builder

WORKDIR /app
COPY . .

RUN cargo build --release

FROM rust:1.82.0-slim-bullseye


WORKDIR /app
COPY --from=builder /app/target/release/deeplx-rs .

CMD ["./deeplx-rs"]
