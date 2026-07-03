FROM node:20-alpine

WORKDIR /app

RUN apk add --no-cache \
    chromium \
    font-noto-cjk \
    freetype \
    harfbuzz \
    nss \
    ttf-freefont

ENV PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH=/usr/bin/chromium-browser \
    PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1

EXPOSE 3000

CMD ["npm", "run", "dev"]
