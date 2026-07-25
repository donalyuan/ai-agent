FROM node:24.10.0-bookworm-slim AS build

WORKDIR /app
COPY services/agent-runtime/package.json services/agent-runtime/package-lock.json ./
RUN npm ci
COPY services/agent-runtime/tsconfig.json services/agent-runtime/tsconfig.build.json ./
COPY services/agent-runtime/src ./src
COPY agent-definitions ./agent-definitions
RUN npm run build

FROM build AS test

COPY services/agent-runtime/tests ./tests
CMD ["npm", "test"]

FROM node:24.10.0-bookworm-slim AS runtime

ENV NODE_ENV=production
ENV NOVEX_AGENT_DEFINITIONS_DIR=/app/agent-definitions
WORKDIR /app
COPY services/agent-runtime/package.json services/agent-runtime/package-lock.json ./
RUN npm ci --omit=dev && npm cache clean --force
COPY --from=build /app/dist ./dist
COPY --from=build /app/agent-definitions ./agent-definitions
RUN mkdir -p /data && chown node:node /data

USER node
EXPOSE 8082
CMD ["node", "dist/main.js"]
