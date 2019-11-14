FROM rustlang/rust:nightly-slim as builder

RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev

# Shut these ****ing ANSI escape off
# For CI's sake
ENV TERM dumb

# https://users.rust-lang.org/t/creating-official-docker-image-for-rust/4165/7
# Cache dependency
ADD Cargo.toml /app/
ADD Cargo.lock /app/
RUN cd /app && \
    mkdir src && \
    touch src/lib.rs && \
    cargo build --color never --release --lib && \
    rm src/lib.rs

COPY . /app/
WORKDIR /app
RUN cargo +nightly build --color never --release

# While alpine is smaller, all of our other images are based off ubuntu 18.04
# Since it is quite likely all these lives on the same machine,
# It won't hurt to use it
FROM ubuntu:18.04

MAINTAINER llk89 @ ShanghaiTech GeekPie Association

EXPOSE 8000

RUN apt-get update && \
    apt-get install -y pkg-config libssl1.1 default-mysql-client && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY . /app
COPY --from=builder /app/target/release/oj-gitlab-middleware /app/oj-gitlab-middleware

CMD /app/entry-point.sh

