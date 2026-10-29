FROM debian:13

COPY ddns4cf /usr/local/bin/ddns4cf

VOLUME ["/config"]

ENTRYPOINT ["/usr/local/bin/ddns4cf", "--config", "/config/config.json5"]