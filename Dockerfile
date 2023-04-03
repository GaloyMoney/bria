FROM clux/muslrust:stable AS build
  COPY . /src
  WORKDIR /src
  RUN SQLX_OFFLINE=true cargo build --release --locked

FROM ubuntu
  RUN apt update && apt install postgresql-client --yes
  COPY --from=build /src/target/x86_64-unknown-linux-musl/debug/bria /usr/local/bin
  RUN mkdir /bria
  RUN chown -R 1000 /bria && chmod -R u+w /bria
  USER 1000
  WORKDIR /bria
  CMD ["bria"]
