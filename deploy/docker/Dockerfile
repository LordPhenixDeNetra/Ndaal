# Image of an ndaal node (T01.22, EF-OPS-07, EXPLOITATION.md section 1.2).
# Build from the repository root:
#   docker build -f deploy/docker/Dockerfile -t ndaal .
# Base images are pinned by digest (CLAUDE.md section 4.12).

# rust:1.99.0-bookworm, the same image as scripts/check-linux.sh.
FROM rust@sha256:114c7a4425406451c2866b6aafe69fe29b1b298832db1277d411ac73c82d04d6 AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p ndaal-cli \
    && cp target/release/ndaal /ndaal \
    && mkdir /data-empty

# gcr.io/distroless/cc-debian12 (multi-architecture index): glibc of the build
# image's Debian release, CA certificates, no shell (ADR-032 context: no
# static musl binary, which would need a C allocator to keep its speed).
FROM gcr.io/distroless/cc-debian12@sha256:e5d81ddde149641e2a9ba55be4545bc125c67de07508b03ba4c22e6eb0ded5aa
COPY --from=build /ndaal /usr/local/bin/ndaal
# The runtime image has no shell to create /data: copy an empty directory
# owned by the node user, so a new named volume inherits that owner.
COPY --from=build --chown=10001:10001 /data-empty /data
USER 10001:10001
# Image settings in a file, not in NDAAL_* variables: variables would beat
# a configuration file mounted over this one. Listening on every interface
# is refused without authentication or an explicit auth.allow_insecure
# (ENF-SEC-04): `docker run` alone stops with a clear message; compose.yaml
# opts in and publishes on the host loopback.
COPY deploy/docker/ndaal.toml /etc/ndaal/ndaal.toml
ENV NDAAL_CONFIG=/etc/ndaal/ndaal.toml
EXPOSE 8080
VOLUME /data
HEALTHCHECK --interval=10s --timeout=6s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/ndaal", "health"]
ENTRYPOINT ["/usr/local/bin/ndaal"]
CMD ["server"]
