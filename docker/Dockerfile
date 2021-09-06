FROM ubuntu:focal-20210827

WORKDIR node

COPY target/release/aleph-node /node/aleph-node
RUN chmod +x aleph-node

COPY docker/docker_entrypoint.sh /node/docker_entrypoint.sh
RUN chmod +x docker_entrypoint.sh

EXPOSE 30333 9933 9944

ENTRYPOINT ["./docker_entrypoint.sh"]
