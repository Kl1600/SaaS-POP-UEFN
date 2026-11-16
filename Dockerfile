# Image de production de POP : interface + serveur, base SQLite dans /data.
FROM node:20-bookworm-slim AS web
WORKDIR /web
COPY web/package*.json ./
RUN npm ci --no-audit --no-fund
COPY web/ ./
RUN npm run build

FROM rust:1-bookworm AS server
WORKDIR /server
COPY server/ ./
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=server /server/target/release/pop-server /app/pop-server
COPY --from=server /server/migrations /app/migrations
COPY --from=web /web/dist /app/web
ENV POP_BIND=0.0.0.0 PORT=8080 POP_WEB_DIR=/app/web DATABASE_URL=sqlite:///data/pop.db POP_SECURE_COOKIES=1
VOLUME /data
EXPOSE 8080
CMD ["/app/pop-server"]
