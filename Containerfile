# Builder: Red Hat hardened Rust image with project MSRV installed
FROM registry.access.redhat.com/hi/rust:1.95-builder AS builder

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH="/usr/local/cargo/bin:${PATH}"

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain 1.96.0 --profile minimal && \
    rustup component add clippy rustfmt llvm-tools-preview

WORKDIR /app
COPY . .

RUN cargo build --release --locked

# Runtime: Red Hat hardened core runtime
FROM registry.access.redhat.com/hi/core-runtime:2.42

COPY --from=builder /usr/lib64/libssl.so.3 /usr/lib64/libcrypto.so.3 /usr/lib64/
COPY --from=builder /app/target/release/thufir /usr/bin/thufir

USER 1001

ENTRYPOINT ["/usr/bin/thufir"]
