# Builder: UBI9 with Rust 1.96 and native TLS build dependencies
FROM registry.access.redhat.com/ubi9/ubi:latest AS builder

RUN dnf install -y gcc openssl-devel perl-FindBin pkg-config && \
    dnf clean all

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH="/usr/local/cargo/bin:${PATH}"

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain 1.96.0 --profile minimal

WORKDIR /app
COPY . .

RUN cargo build --release --locked

# Runtime: UBI9 minimal with OpenSSL for native TLS
FROM registry.access.redhat.com/ubi9/ubi-minimal:latest

COPY --from=builder /app/target/release/thufir /usr/bin/thufir

USER 1001

ENTRYPOINT ["/usr/bin/thufir"]
