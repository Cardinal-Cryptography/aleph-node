FROM rustlang/rust:nightly-slim

WORKDIR subxt

RUN cargo install subxt-cli
RUN rustup component add rustfmt --toolchain nightly

COPY docker/docker_entrypoint.sh /subxt/docker_entrypoint.sh
COPY src/aleph_zero.rs /subxt/aleph_zero_current.rs
COPY rustfmt.toml /subxt/rustfmt.toml

RUN chmod +x /subxt/docker_entrypoint.sh
RUN rustc --version

ENTRYPOINT ["./docker_entrypoint.sh"]
