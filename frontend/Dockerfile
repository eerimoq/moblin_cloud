FROM --platform=$BUILDPLATFORM node:22-slim AS builder
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM caddy:2
ENV BACKEND_HOST=backend:8080
COPY Caddyfile /etc/caddy/Caddyfile
COPY --from=builder /app/dist /srv
