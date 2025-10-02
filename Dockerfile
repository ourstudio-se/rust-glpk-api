# syntax=docker/dockerfile:1

# Comments are provided throughout this file to help you get started.
# If you need more help, visit the Dockerfile reference guide at
# https://docs.docker.com/go/dockerfile-reference/

# Want to help us make this template better? Share your feedback here: https://forms.gle/ybq9Krt8jtBL3iCk7

ARG RUST_VERSION=1.86.0
ARG APP_NAME=rust-glpk
ARG GLPK_VER=5.0

################################################################################
# Create a stage for building the application.

FROM rust:${RUST_VERSION}-alpine AS build
ARG APP_NAME
ARG GLPK_VER
WORKDIR /app

RUN apk add --no-cache clang lld musl-dev git curl make pkgconf
RUN apk add --no-cache glpk-dev gmp-dev

# Create glpk.pc file manually because it's not included by default in alpine
RUN mkdir -p /usr/local/lib/pkgconfig && \
    cat > /usr/local/lib/pkgconfig/glpk.pc <<EOF
prefix=/usr
libdir=\${prefix}/lib
includedir=\${prefix}/include

Name: glpk
Description: GNU Linear Programming Kit
Version: ${GLPK_VER}
Libs: -L\${libdir} -lglpk
Libs.private: -lgmp
Cflags: -I\${includedir}
EOF

RUN cat /usr/local/lib/pkgconfig/glpk.pc

# Add the custom glpk.pc file to the path
ENV PKG_CONFIG_PATH=/usr/local/lib/pkgconfig:/usr/lib/pkgconfig

# Viktigt för att kunna använda ${TARGETPLATFORM} i RUN
ARG TARGETPLATFORM

RUN --mount=type=bind,source=src,target=src \
    --mount=type=bind,source=static,target=static \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    --mount=type=cache,id=cargo-target-${TARGETPLATFORM},target=/app/target/ \
    --mount=type=cache,id=cargo-git-${TARGETPLATFORM},target=/usr/local/cargo/git/db \
    --mount=type=cache,id=cargo-registry-${TARGETPLATFORM},target=/usr/local/cargo/registry/ \
    cargo build --locked --release && \
    cp ./target/release/$APP_NAME /bin/server

################################################################################
# Create a new stage for running the application that contains the minimal
# runtime dependencies for the application. This often uses a different base
# image from the build stage where the necessary files are copied from the build
# stage.
#
# The example below uses the alpine image as the foundation for running the app.
# By specifying the "3.18" tag, it will use version 3.18 of alpine. If
# reproducability is important, consider using a digest
# (e.g., alpine@sha256:664888ac9cfd28068e062c991ebcff4b4c7307dc8dd4df9e728bedde5c449d91).
FROM alpine:3.18 AS final

# Create a non-privileged user that the app will run under.
# See https://docs.docker.com/go/dockerfile-user-best-practices/
ARG UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser
USER appuser

# Copy the executable from the "build" stage.
COPY --from=build /bin/server /bin/

# Expose the port that the application listens on.
EXPOSE 9000

# What the container should run when it is started.
CMD ["/bin/server"]
