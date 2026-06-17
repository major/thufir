# Builder: Red Hat hardened Rust image with project MSRV installed
FROM registry.access.redhat.com/hi/rust:1.95-builder@sha256:6c9eb898c3b0ba9d9c9efe9b3368d96cfede890b1b65cc243d6e9669d5414987 AS builder

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
FROM registry.access.redhat.com/hi/core-runtime:2.42@sha256:dcd72eaa2df901c4915e1eec915906c8787c64b9e4149b4211d4500fbbe71791

COPY --from=builder /usr/lib64/libssl.so.3 /usr/lib64/libcrypto.so.3 /usr/lib64/
COPY --from=builder /app/target/release/thufir /usr/bin/thufir

USER 1001

ENTRYPOINT ["/usr/bin/thufir"]
