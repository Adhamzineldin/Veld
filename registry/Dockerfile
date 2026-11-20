# Build:  docker build -t veld-registry registry/
FROM golang:1.26-alpine AS build
WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o /out/veld-registry ./cmd/veld-registry

FROM alpine:3.20
RUN adduser -D -u 10001 veld && mkdir /data && chown veld /data
USER veld
WORKDIR /data
COPY --from=build /out/veld-registry /usr/local/bin/veld-registry
ENV VELD_ADDR=:8080 VELD_STORAGE=/data/packages
EXPOSE 8080
VOLUME /data
ENTRYPOINT ["veld-registry"]
