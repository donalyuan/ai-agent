FROM node:22.12.0-alpine3.20 AS build
WORKDIR /workspace
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/web/package.json apps/web/package.json
COPY packages/contracts/package.json packages/contracts/package.json
# Node 22.12 的内置 Corepack 缺少当前 registry 签名密钥；固定安装同一 pnpm 版本。
RUN npm install --global pnpm@10.19.0 && pnpm install --frozen-lockfile
COPY apps/web apps/web
COPY packages/contracts packages/contracts
RUN pnpm --filter @video-agent/web build

FROM nginx:1.27.4-alpine
COPY --from=build /workspace/apps/web/dist /usr/share/nginx/html
COPY apps/web/nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
