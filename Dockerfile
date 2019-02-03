FROM rustlang/rust:nightly-slim

MAINTAINER llk89 @ ShanghaiTech GeekPie Association

EXPOSE 8000

# https://users.rust-lang.org/t/creating-official-docker-image-for-rust/4165/7
# Cache dependency
ADD Cargo.toml /app/
ADD Cargo.lock /app/
RUN cd /app && \
    mkdir src && \
    touch src/lib.rs && \
    cargo build --release --lib

COPY . /app/
WORKDIR /app
RUN cargo +nightly build --release

CMD entry-point.sh
