FROM rustlang/rust:nightly-slim

MAINTAINER llk89 @ ShanghaiTech GeekPie Association

EXPOSE 8000

COPY . /app/
WORKDIR /app
RUN cargo build --releases

CMD cargo run --releases
