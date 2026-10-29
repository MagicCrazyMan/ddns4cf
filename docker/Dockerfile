FROM debian:13 AS ca-certificates

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

FROM debian:13

COPY --from=ca-certificates /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

COPY ddns4cf /usr/local/bin/ddns4cf

VOLUME ["/config"]

ENTRYPOINT ["/usr/local/bin/ddns4cf", "--config", "/config/config.toml"]