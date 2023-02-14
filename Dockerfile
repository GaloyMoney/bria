FROM clux/muslrust:stable AS build
  COPY . /src
  WORKDIR /src
  RUN SQLX_OFFLINE=true cargo build --locked

FROM ubuntu
  COPY --from=build /src/target/x86_64-unknown-linux-musl/debug/bria /usr/local/bin
  USER 1000
  CMD ["bria"]