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

# Only copy stuff necessary for a build
COPY src /app/
WORKDIR /app
RUN cargo +nightly build --color never --release

# Copy everything else
COPY . /app/
RUN chmod +x /app/entry-point.sh

CMD /app/entry-point.sh
