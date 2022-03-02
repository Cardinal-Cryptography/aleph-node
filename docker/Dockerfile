FROM ubuntu:impish-20220128

RUN apt update && \
        apt install curl -y && \
        apt clean && \
        rm -rf /var/lib/apt/lists/*

EXPOSE 30333 9933 9944

WORKDIR node

COPY target/release/aleph-node /usr/local/bin
RUN chmod +x /usr/local/bin/aleph-node

COPY docker/docker_entrypoint.sh /node/docker_entrypoint.sh
RUN chmod +x /node/docker_entrypoint.sh

ENTRYPOINT ["./docker_entrypoint.sh"]
