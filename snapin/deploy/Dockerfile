FROM node:22-alpine

WORKDIR /app

COPY server/package.json server/package-lock.json* ./
RUN npm ci --omit=dev

COPY server/ ./

EXPOSE 3300

ENV NODE_ENV=production
ENV PORT=3300

CMD ["node", "index.js"]
