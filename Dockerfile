FROM rust:1.87.0-alpine3.22
RUN apk add libressl-dev
RUN apk add musl-dev

WORKDIR /usr/src/nicotine-ingest
COPY . .

RUN cargo install --path .
RUN ls .
CMD [ "nicotine-ingest", "RUST_LOG=info"]