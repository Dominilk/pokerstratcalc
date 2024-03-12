FROM rust:latest AS build

WORKDIR /usr/src
COPY . .

WORKDIR /usr/src
RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app
COPY --from=build /usr/src/target/release/poker-client /app/poker-client

ENTRYPOINT [ "/app/poker-client" ]