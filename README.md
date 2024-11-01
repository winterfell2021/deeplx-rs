# DeepLX-rs

## Quick Start

### Using Docker

1. Clone the repository
2. Create a `docker-compose.yaml` file with your DeepL session token:
```
version: '3'

services:
  deepl-api:
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "59000:59000"
    environment:
      - PRIMARY_PORT=59000
      - DL_SESSION=
    restart: unless-stopped
```