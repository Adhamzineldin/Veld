# The React app: built with Vite, served by nginx, which also routes /api/<service> to the
# services (so the browser talks to one origin).
FROM node:22-alpine AS build
WORKDIR /app
COPY package.json package-lock.json tsconfig.json vite.config.mts index.html ./
COPY src ./src
RUN npm ci
RUN npm run build

FROM nginx:1.27-alpine
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/dist /usr/share/nginx/html
EXPOSE 80
