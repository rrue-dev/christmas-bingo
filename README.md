# christmas-bingo

A family-friendly, LAN-only multiplayer Christmas Bingo. Players scan a QR code and play in their phone's browser (or Expo Go). The host machine runs the server and the SQLite database.

```
app/     Expo (TypeScript, expo-router): web + native client
server/  Rust: Axum + (SQLx + SQLite from M1)
data/    SQLite migrations, card sets, runtime DB file
docs/    Design docs: start with docs/implementation-plan.md
```

## Prerequisites

- Rust (stable) with cargo
- Node.js 20+ and npm

## Development

Run the server (http://localhost:8080):

```powershell
cd server; cargo run
```

Run the app in a browser with hot reload (Metro on http://localhost:8081, calls the server at http://localhost:8080):

```powershell
cd app; npm install; npm run web
```

Debug builds of the server allow CORS from `http://localhost:8081`, so the Metro dev page can reach the API.

For Expo Go on a phone, point the app at the host's LAN IP:

```powershell
cd app; $env:EXPO_PUBLIC_API_BASE_URL = "http://192.168.1.50:8080"; npm start
```

## Serve everything from one URL (how players will use it)

```powershell
cd app; npm run export:web
```

```powershell
cd server; cargo run --release
```

The server serves `app/dist` at `/` and the API at `/api/v1`. Open `http://<host-ip>:8080`.

## Tests

```powershell
cd server; cargo test
```

```powershell
cd app; npm test; npm run typecheck
```

## Server configuration

| Variable | Default | Purpose |
|---|---|---|
| `BIND_ADDR` | `0.0.0.0:8080` | Listen address |
| `PUBLIC_URL` | — | Fixed LAN URL used for the QR code |
| `DATA_DIR` | `../data` | SQLite + card sets |
| `WEB_DIR` | `../app/dist` | Exported Expo Web bundle |
| `DEV_CORS_ORIGINS` | `http://localhost:8081` in debug builds, none in release | Comma-separated CORS allow-list |
| `RUST_LOG` | `info` | Log filter |
