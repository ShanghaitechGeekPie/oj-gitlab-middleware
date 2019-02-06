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
    cargo build --release --lib --color=never -- --color=never && \
    rm src/lib.rs

COPY . /app/
WORKDIR /app
RUN cargo +nightly build --release --color=never -- --color=never

CMD entry-point.sh
