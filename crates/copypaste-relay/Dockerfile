# Build from the repository root:
#   docker build -f crates/copypaste-relay/Dockerfile -t copypaste-relay .
FROM rust:1-slim AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p copypaste-relay

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/copypaste-relay /usr/local/bin/copypaste-relay
EXPOSE 8080 3478/udp
ENTRYPOINT ["/usr/local/bin/copypaste-relay"]
