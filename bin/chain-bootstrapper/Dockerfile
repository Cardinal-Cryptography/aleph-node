FROM ubuntu:jammy

COPY target/release/chain-bootstrapper /usr/local/bin
RUN chmod +x /usr/local/bin/chain-bootstrapper

CMD ["/usr/local/bin/chain-bootstrapper"]
