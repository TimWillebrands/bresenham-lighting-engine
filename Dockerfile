FROM debian:trixie-slim AS build

RUN apt-get update \
    && apt-get install -y wget \
    && apt-get install -y libxml2

RUN wget https://github.com/c3lang/c3c/releases/download/latest/c3-linux.tar.gz \
    && tar -xf  c3-linux.tar.gz \
    && rm c3-linux.tar.gz

COPY *.c3 ./

RUN c3/c3c compile -D PLATFORM_WEB --reloc=none --target wasm32 -O5 -g0 --link-libc=no  --no-entry -o lighting -z --export-table arctan.c3 ray.c3 lighting.c3

FROM debian:trixie-slim AS final
COPY --from=build /lighting.wasm /lighting.wasm
