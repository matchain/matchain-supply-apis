FROM rust:1.88 as builder

WORKDIR /matchain-supply-apis
COPY . .

RUN cargo build --release

FROM ubuntu:22.04

COPY --from=builder /matchain-supply-apis/target/release/matchain-supply-apis /usr/bin/matchain-supply-apis

RUN chmod +x /usr/bin/matchain-supply-apis

ENTRYPOINT [ "/usr/bin/matchain-supply-apis" ]