FROM rustlang/rust:nightly-slim

MAINTAINER llk89 @ ShanghaiTech GeekPie Association

EXPOSE 8000

RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev

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

CMD entry-point.sh
