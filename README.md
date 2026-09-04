# Apple Maps Tile Proxy

High-performance, asynchronous Apple Maps tile proxy with offline cryptographic URL signing (AES-256-CBC, PKCS7, SHA-256) and concurrent hybrid tile compositing.

## Features

- **Cryptographic Signing**: Apple's internal AES-256-CBC URL signing scheme (`sid` + `accessKey`).
- **High Concurrency & Async I/O**: Built on [Axum](https://github.com/tokio-rs/axum) and [Tokio](https://tokio.rs/) with connection pooling and HTTP/2 keep-alive.
- **Concurrent Hybrid Compositing**: Satellite JPEG and overlay PNG tiles are fetched concurrently and alpha-composited with Lanczos3 resampling in worker threads.
- **Complete Endpoint Coverage**:
  - Standard Road tiles (Light & Dark mode, customizable scale, language, POI, labels, emphasis)
  - Satellite imagery
  - Hybrid tiles (Satellite + Transparent Overlay)
  - Overlay vector tiles
  - MapKit icons and shields
  
---

## Build & Run

### Prerequisites
- [Rust](https://rustup.rs/) (1.80+ recommended)

### Development
```bash
cargo run
```

### Production Build
```bash
cargo build --release
./target/release/apple-map-proxy.exe --port 8080
```

### Docker

#### Using Docker Compose (Recommended for Homelab)
```bash
docker compose up -d
```

#### Using Docker CLI
```bash
# Run pre-built image from GitHub Container Registry
docker run -d \
  --name apple-map-proxy \
  --restart unless-stopped \
  -p 8080:8080 \
  ghcr.io/anson10124/apple_map_proxy:latest

# Or build and run locally
docker build -t apple-map-proxy .
docker run -d --name apple-map-proxy -p 8080:8080 apple-map-proxy
```

---

## Command-Line Arguments & Environment Variables

| Flag | Env Var | Default | Description |
|------|---------|---------|-------------|
| `-H, --host` | `HOST` | `0.0.0.0` | Host IP address to bind to |
| `-p, --port` | `PORT` | `8080` | Port number to listen on |
| `--secret-prefix` | `SECRET_PREFIX` | `4cjLaD4jGRwlQ9U72xIzEBe0vHBmf9` | Cryptographic secret key prefix |
| `--satellite-version` | `SATELLITE_VERSION` | `10421` | Satellite layer version |
| `--overlay-version` | `OVERLAY_VERSION` | `2609034` | Overlay layer version |
| `--road-version` | `ROAD_VERSION` | `2609034` | Road layer version |

---

## API Endpoints

### 1. Road Map (Light Mode)
```http
GET /road/{z}/{x}/{y}
GET /standard/{z}/{x}/{y}
```
*Optional Query Parameters*:
- `tint`: `light` (default) or `dark`
- `scale`: `1` (default) or `2` (retina)
- `lang`: Language code (default: `en`)
- `poi`: `true` / `1` (default) or `false` / `0`
- `labels`: `true` / `1` (default) or `false` / `0`
- `emphasis`: `standard` (default) or `muted`

### 2. Road Map (Dark Mode)
```http
GET /dark/{z}/{x}/{y}
GET /road-dark/{z}/{x}/{y}
```
*Optional Query Parameters*: `scale`, `lang`, `poi`, `labels`, `emphasis`.

### 3. Satellite Tiles
```http
GET /satellite/{z}/{x}/{y}
GET /{z}/{x}/{y}
```
Returns JPEG satellite imagery. Extensions such as `.jpg` or `.png` in `{y}` are automatically handled.

### 4. Hybrid Tiles
```http
GET /hybrid/{z}/{x}/{y}
```
Fetches satellite and overlay tiles concurrently and composites them into an image (`image/jpeg`).

### 5. Overlay Tiles
```http
GET /overlay/{z}/{x}/{y}
```
Returns transparent vector overlay tiles (`image/png`).

### 6. MapKit Icons & Shields
```http
GET /md/v1/icon?...
GET /md/v1/shield?...
```
Proxies MapKit metadata resources with cryptographic signing.
