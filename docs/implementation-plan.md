# Christmas Bingo — v1 Implementation Plan

LAN-only multiplayer Bingo. Expo client (web + native), Rust/Axum/SQLx/SQLite backend on the host machine, one shared game room, join code + display name.

> **Repo state:** only `README.md` exists today. `app/`, `data/`, `server/` are created in Milestone 0.

---

## Key v1 decisions (read these first)

| Decision | Choice | Why |
|---|---|---|
| Who "calls" items | **The host calls items** (random draw from the set, or tap a specific item when it happens, e.g. a movie scene appears) | The server can only check a claim against a list of called items it knows about. Without calls, "server validates" has nothing to check. |
| How a claim is judged | Server works out the player's marked cells from **its own call log**. What the client has marked is only for the screen and never decides the result. | One source of truth, no disputes, simple tests |
| What "no-bingo" means | The player says "I don't have bingo". The server confirms it, or replies "actually, you do!" | Gives the two claim kinds required. Fun for kids. (Open question 1) |
| Winning patterns | Any full row, column or diagonal on a 5×5 card with a free centre | Classic, simple to explain |
| Updates | Client **polls** every 2–3 s | No WebSocket complexity. Load on a LAN is trivial. |
| How the web app is served | Axum serves the exported Expo Web bundle **from the same origin** as the API | One URL, one QR code, no CORS in production |
| Card sets | Authored as **files** (JSON manifest + assets) in `data/card-sets/`, imported into SQLite at startup | Supports many sets now. An in-app designer can come later without changing the schema. |
| First demo set | `christmas-icons`: family-friendly Christmas **emoji** tiles | No licensing issues, no image pipeline, renders everywhere |

---

## 1) Proposed architecture

```
christmas-bingo/
├── app/      Expo (TypeScript, expo-router). Web + native from one codebase
├── server/   Rust crate: Axum HTTP, rules engine, SQLx data access
├── data/     SQLite concerns: migrations, card-set sources, runtime DB file
└── docs/
```

### `app/` — client
- Screens: `join` (code + name) → `card` (5×5 grid, daubing, claim buttons, result modal); host screens: `host/new`, `host/lobby` (join code + QR), `host/game` (call next / call specific item, claims feed, end game).
- `src/api/` is a typed fetch client for `/api/v1`. Base URL = `window.location.origin` on web, `EXPO_PUBLIC_API_BASE_URL` on native.
- `src/storage/` is a small wrapper: `localStorage` on web, `expo-secure-store` on native. It holds the player or host token.
- Christmas theme: red/green/gold palette, light snowfall animation (switched off when the device asks for reduced motion), large tap targets (≥ 56 px) for kids.
- Daubing (marking a tile) happens only on the device. Called items get a soft glow to help younger players.

### `server/` — backend
```
server/src/
├── main.rs          config, startup, migrations, card-set import, QR print, serve
├── config.rs        BIND_ADDR, PUBLIC_URL, DATA_DIR, HOST_PIN, DEV_CORS_ORIGINS
├── http/            routers, extractors (PlayerAuth, HostAuth), error mapping
├── rules/           PURE: card generation, line detection, claim evaluation (no I/O)
├── repo/            SQLx queries, one module per aggregate (games, players, calls, claims, card_sets)
├── card_sets/       manifest parsing + validation + idempotent import
└── static_files.rs  ServeDir for app/dist with SPA fallback to index.html
```
- Crates: `axum`, `tokio`, `sqlx` (sqlite, runtime-tokio, macros, migrate), `serde`, `tower-http` (fs, cors, limit, timeout, trace), `tracing`, `rand`, `sha2`, `qrcode`, `time` or `chrono`, `thiserror`, `tower_governor` (rate limiting).

### `data/` — local SQLite concerns
```
data/
├── migrations/                 0001_init.sql ...   (embedded via sqlx::migrate!("../data/migrations"))
├── card-sets/
│   └── christmas-icons/set.json
├── card-sets-private/          gitignored: personal sets (e.g. movie stills for family use)
├── bingo.db                    gitignored runtime DB (WAL mode)
└── README.md                   card-set authoring guide
```
The server owns *code*. `data/` owns *schema, seed content and the DB file*. `DATA_DIR` is configurable, so the DB can live somewhere else if needed.

---

## 2) Data model (SQLite)

Pragmas at connect: `journal_mode=WAL`, `foreign_keys=ON`, `busy_timeout=5000`. Timestamps are `TEXT` in RFC 3339 UTC.

```sql
CREATE TABLE card_sets (
  id            INTEGER PRIMARY KEY,
  slug          TEXT NOT NULL UNIQUE,          -- 'christmas-icons'
  name          TEXT NOT NULL,
  description   TEXT,
  version       INTEGER NOT NULL,              -- bump to re-import
  free_center   INTEGER NOT NULL DEFAULT 1,
  style_json    TEXT,                          -- theme hints: colors, background, font
  created_at    TEXT NOT NULL,
  updated_at    TEXT NOT NULL
);

CREATE TABLE card_set_items (
  id            INTEGER PRIMARY KEY,
  card_set_id   INTEGER NOT NULL REFERENCES card_sets(id),
  key           TEXT NOT NULL,                 -- stable id within set: 'snowman'
  label         TEXT NOT NULL,                 -- 'Snowman'
  icon_kind     TEXT NOT NULL CHECK (icon_kind IN ('emoji','image','text')),
  icon_value    TEXT NOT NULL,                 -- '⛄' | 'images/snowman.webp' | 'Ho ho ho!'
  sort_order    INTEGER NOT NULL,
  retired       INTEGER NOT NULL DEFAULT 0,    -- soft-delete on re-import so old games stay valid
  UNIQUE (card_set_id, key)
);

CREATE TABLE games (
  id              INTEGER PRIMARY KEY,
  card_set_id     INTEGER NOT NULL REFERENCES card_sets(id),
  join_code       TEXT NOT NULL,
  status          TEXT NOT NULL CHECK (status IN ('lobby','active','finished')),
  host_token_hash BLOB NOT NULL,
  grid_size       INTEGER NOT NULL DEFAULT 5,
  rules_version   INTEGER NOT NULL DEFAULT 1,
  created_at      TEXT NOT NULL,
  started_at      TEXT,
  ended_at        TEXT
);
-- Enforces "single shared room": at most one open game.
CREATE UNIQUE INDEX one_open_game ON games ((1)) WHERE status IN ('lobby','active');

CREATE TABLE players (
  id                INTEGER PRIMARY KEY,
  game_id           INTEGER NOT NULL REFERENCES games(id),
  display_name      TEXT NOT NULL,
  display_name_norm TEXT NOT NULL,             -- trimmed, NFC, lowercased
  token_hash        BLOB NOT NULL,
  joined_at         TEXT NOT NULL,
  last_seen_at      TEXT NOT NULL,
  UNIQUE (game_id, display_name_norm)
);

CREATE TABLE player_cards (
  player_id     INTEGER PRIMARY KEY REFERENCES players(id),
  game_id       INTEGER NOT NULL REFERENCES games(id),
  seed          INTEGER NOT NULL,
  cells_json    TEXT NOT NULL,                 -- [item_id|null x 25], row-major; null = free centre
  created_at    TEXT NOT NULL
);

CREATE TABLE calls (
  id            INTEGER PRIMARY KEY,
  game_id       INTEGER NOT NULL REFERENCES games(id),
  item_id       INTEGER NOT NULL REFERENCES card_set_items(id),
  seq           INTEGER NOT NULL,              -- 1,2,3... per game
  called_at     TEXT NOT NULL,
  UNIQUE (game_id, item_id),
  UNIQUE (game_id, seq)
);

CREATE TABLE claims (
  id                 INTEGER PRIMARY KEY,
  game_id            INTEGER NOT NULL REFERENCES games(id),
  player_id          INTEGER NOT NULL REFERENCES players(id),
  kind               TEXT NOT NULL CHECK (kind IN ('bingo','no_bingo')),
  client_marks_json  TEXT,                     -- what the player had marked, for later analysis only
  call_seq_at_claim  INTEGER NOT NULL,         -- snapshot the claim was judged against
  submitted_at       TEXT NOT NULL
);

CREATE TABLE claim_outcomes (
  claim_id          INTEGER PRIMARY KEY REFERENCES claims(id),
  result            TEXT NOT NULL CHECK (result IN ('valid','invalid')),
  reason            TEXT NOT NULL,             -- see section 4
  winning_line_json TEXT,                      -- e.g. {"type":"row","index":2,"cells":[10,11,12,13,14]}
  rules_version     INTEGER NOT NULL,
  evaluated_at      TEXT NOT NULL
);

CREATE INDEX idx_calls_game_seq   ON calls(game_id, seq);
CREATE INDEX idx_claims_game      ON claims(game_id, submitted_at);
CREATE INDEX idx_players_game     ON players(game_id);
```

**Relationships:** card_set 1–N items · card_set 1–N games · game 1–N players · player 1–1 card · game 1–N calls · player 1–N claims · claim 1–1 outcome.

`claim_outcomes` is a separate table, as the requirements ask. It also keeps room for re-evaluating claims under a new `rules_version` later (deeper state analysis).

### Card-set manifest (`data/card-sets/<slug>/set.json`)
```json
{
  "slug": "christmas-icons",
  "name": "Christmas Icons",
  "description": "Family-friendly Christmas symbols",
  "version": 1,
  "freeCenter": true,
  "style": { "primary": "#C8102E", "secondary": "#0B6623", "accent": "#F2C14E", "freeLabel": "🎄 FREE" },
  "items": [
    { "key": "santa",        "label": "Santa",          "emoji": "🎅" },
    { "key": "tree",         "label": "Christmas Tree", "emoji": "🎄" },
    { "key": "snowman",      "label": "Snowman",        "emoji": "⛄" },
    { "key": "reindeer",     "label": "Reindeer",       "emoji": "🦌" },
    { "key": "gift",         "label": "Present",        "emoji": "🎁" },
    { "key": "bell",         "label": "Bell",           "emoji": "🔔" },
    { "key": "star",         "label": "Star",           "emoji": "⭐" },
    { "key": "snowflake",    "label": "Snowflake",      "emoji": "❄️" },
    { "key": "cookie",       "label": "Cookie",         "emoji": "🍪" },
    { "key": "milk",         "label": "Milk",           "emoji": "🥛" },
    { "key": "stocking",     "label": "Stocking",       "emoji": "🧦" },
    { "key": "candle",       "label": "Candle",         "emoji": "🕯️" },
    { "key": "sled",         "label": "Sled",           "emoji": "🛷" },
    { "key": "elf",          "label": "Elf",            "emoji": "🧝" },
    { "key": "candy",        "label": "Candy",          "emoji": "🍬" },
    { "key": "carols",       "label": "Carols",         "emoji": "🎶" },
    { "key": "scarf",        "label": "Scarf",          "emoji": "🧣" },
    { "key": "mittens",      "label": "Mittens",        "emoji": "🧤" },
    { "key": "bow",          "label": "Bow",            "emoji": "🎀" },
    { "key": "lights",       "label": "Lights",         "emoji": "💡" },
    { "key": "house",        "label": "Cozy House",     "emoji": "🏠" },
    { "key": "mrs-claus",    "label": "Mrs. Claus",     "emoji": "🤶" },
    { "key": "angel",        "label": "Angel",          "emoji": "👼" },
    { "key": "hot-cocoa",    "label": "Hot Cocoa",      "emoji": "☕" },
    { "key": "fireplace",    "label": "Fireplace",      "emoji": "🔥" },
    { "key": "gingerbread",  "label": "Gingerbread",    "emoji": "🍪" },
    { "key": "snowman-snow", "label": "Snowy Day",      "emoji": "☃️" },
    { "key": "wreath",       "label": "Wreath",         "emoji": "🎄" },
    { "key": "moon",         "label": "Christmas Eve",  "emoji": "🌙" },
    { "key": "shining-star", "label": "Shining Star",   "emoji": "🌟" }
  ]
}
```
Items take exactly one of `emoji` | `image` (a path relative to the set folder) | `text`. Before the demo, swap out the few duplicate glyphs (🍪/🎄 are each used twice). Every tile also shows its label, so it still reads clearly if a glyph renders differently.

**Import rules:** at least 24 active items (for a 5×5 card with a free centre); keys unique; image files must exist and be ≤ 512 KB in png/webp/jpg. The import is idempotent: a set is upserted only when `version` goes up, and removed items are marked `retired=1`, never deleted. Image assets are served at `/assets/card-sets/<slug>/<path>`.

A future "Christmas Movie Scenes" set uses `image` items. It lives in `data/card-sets-private/` (gitignored), because movie stills are copyrighted. They are fine for private family use but should not be committed or redistributed.

---

## 3) API design (`/api/v1`, JSON)

**Auth:** `Authorization: Bearer <token>`. Player tokens and host tokens are 128-bit random values, base64url-encoded. Only SHA-256 hashes are stored. Tokens never go in URLs.

| Method & path | Auth | Purpose |
|---|---|---|
| `GET /health` | – | `{ "status":"ok", "version":"0.1.0" }` |
| `GET /card-sets` | – | `[{ slug, name, description, itemCount }]` |
| `GET /card-sets/{slug}` | – | set with items and style |
| `POST /games` | Host PIN (`X-Host-Pin`) | Create game. 409 if one is open. |
| `GET /games/current` | – | Discover the open game: `{ id, status, cardSet:{slug,name}, playerCount }` (never returns the join code) |
| `POST /games/{id}/players` | – | Join with code + name. Returns token + card. |
| `GET /games/{id}/me` | Player | Rejoin/refresh: player, card, calls, my claims |
| `GET /games/{id}/state?sinceSeq=N` | Player or Host | Poll: status, new calls, recent results |
| `POST /games/{id}/start` | Host | lobby → active |
| `POST /games/{id}/calls` | Host | Call an item (random or by key) |
| `POST /games/{id}/claims` | Player | Submit a bingo / no_bingo claim. Judged synchronously. |
| `GET /games/{id}/claims` | Host | All claims + outcomes |
| `POST /games/{id}/end` | Host | → finished |
| `DELETE /games/{id}/players/{playerId}` | Host | Kick (also frees the name for a rejoin) |

Game-scoped paths (`/games/{id}/…`) keep a later move to multiple rooms possible without a v2 API. "Single room" is enforced by the DB index, not by the URL shape.

### Shapes

```jsonc
// POST /games   (header X-Host-Pin: 4821)
{ "cardSetSlug": "christmas-icons" }
// 201
{ "game": { "id": 7, "status": "lobby", "joinCode": "SNOW42", "cardSet": { "slug": "christmas-icons", "name": "Christmas Icons" } },
  "hostToken": "q9V..." }

// POST /games/7/players
{ "joinCode": "snow42", "displayName": "Grandma" }
// 201
{ "player": { "id": 31, "displayName": "Grandma" },
  "token": "Zx3...",
  "card": { "gridSize": 5, "cells": [ { "index": 0, "item": { "key": "santa", "label": "Santa", "iconKind": "emoji", "iconValue": "🎅" } },
                                      /* ... */ { "index": 12, "free": true } /* ... */ ] } }

// GET /games/7/state?sinceSeq=4
{ "status": "active", "lastSeq": 6,
  "calls": [ { "seq": 5, "itemKey": "snowman", "calledAt": "2026-12-24T19:02:11Z" },
             { "seq": 6, "itemKey": "bell",    "calledAt": "2026-12-24T19:02:40Z" } ],
  "winners": [ { "playerId": 31, "displayName": "Grandma", "claimId": 88, "at": "2026-12-24T19:03:02Z" } ] }

// POST /games/7/calls      { "mode": "random" }  |  { "mode": "item", "itemKey": "snowman" }
// 201 { "seq": 7, "itemKey": "elf", "label": "Elf", "iconValue": "🧝" }

// POST /games/7/claims
{ "kind": "bingo", "clientMarks": [0,1,2,3,4,12] }
// 201
{ "claim": { "id": 88, "kind": "bingo", "callSeqAtClaim": 6 },
  "outcome": { "result": "valid", "reason": "LINE_COMPLETE",
               "winningLine": { "type": "row", "index": 0, "cells": [0,1,2,3,4] } } }
```

Clients match calls to items by `itemKey`. Items come from `/card-sets/{slug}` and are cached, so poll responses stay small.

### Error model
```json
{ "error": { "code": "JOIN_CODE_INVALID", "message": "That join code doesn't match the current game.", "details": {} } }
```
| HTTP | Codes |
|---|---|
| 400 | `VALIDATION_FAILED` (details lists fields), `DISPLAY_NAME_INVALID` |
| 401 | `AUTH_REQUIRED`, `TOKEN_INVALID`, `HOST_PIN_INVALID` |
| 403 | `NOT_HOST`, `NOT_IN_GAME` |
| 404 | `GAME_NOT_FOUND`, `CARD_SET_NOT_FOUND`, `ITEM_NOT_FOUND` |
| 409 | `GAME_ALREADY_OPEN`, `DISPLAY_NAME_TAKEN`, `GAME_NOT_ACTIVE`, `GAME_NOT_IN_LOBBY`, `ITEM_ALREADY_CALLED`, `ALL_ITEMS_CALLED`, `JOIN_CODE_INVALID` |
| 429 | `RATE_LIMITED`, `CLAIM_COOLDOWN` (details: `retryAfterMs`) |
| 500 | `INTERNAL` (logged with request id; the message never includes internals) |

`message` is always friendly enough to show a child as-is.

---

## 4) Claim validation rules

Everything below runs in **one `BEGIN IMMEDIATE` transaction**, so the call snapshot and the stored claim always match.

**Preconditions (rejected with an error, no claim stored):**
1. The token resolves to a player in game `{id}`, else `NOT_IN_GAME`.
2. Game `status = 'active'`, else `GAME_NOT_ACTIVE`.
3. `kind ∈ {bingo, no_bingo}`. If `clientMarks` is present, it holds ints 0–24 with no duplicates (≤ 25), else `VALIDATION_FAILED`.
4. Cooldown: after an **invalid `bingo`** claim, that player must wait 10 s, else `CLAIM_COOLDOWN`. This stops button-mashing.

**Evaluation (claim + outcome always stored):**
1. `called = { item_id | calls.game_id = id }`, `snapshot = max(seq)`.
2. `marked[i] = cell i is free OR cells[i] ∈ called`.
3. Check the 12 lines: 5 rows, 5 columns, 2 diagonals. `lines = [line | all marked]`.
4. Outcomes:

| kind | lines found | result | reason |
|---|---|---|---|
| bingo | ≥ 1 | valid | `LINE_COMPLETE` (returns the first line in row → col → diag order) |
| bingo | 0 | invalid | `NO_COMPLETE_LINE` (details: closest line + how many cells are missing) |
| no_bingo | 0 | valid | `CONFIRMED_NO_LINE` |
| no_bingo | ≥ 1 | invalid | `HAS_UNCLAIMED_BINGO` (returns the line: "Check again, you have Bingo!") |

5. `clientMarks` is stored and **never** affects the result. A mismatch is recorded only for later analysis.
6. A player can have at most one valid `bingo` per game. A repeat returns the existing outcome with reason `ALREADY_WON`.
7. The game **stays active** after a win, so several family members can win. The host ends it (Open question 3).

`rules/` is a pure module, `evaluate(card, called) -> Outcome`, with table-driven unit tests: every one of the 12 lines, free centre, near-miss, no calls, all called. Card generation is `generate(items, seed) -> cells`: a seeded shuffle that takes the first 24 active items, so cards can be rebuilt and checked in tests.

---

## 5) LAN deployment plan

**Topology:** one Windows host runs `bingo-server.exe`. It serves the API, the exported Expo Web bundle and card-set assets on **one port**.

1. **Fixed URL.** Create a DHCP reservation on the router for the host, e.g. `192.168.1.50`, and set `PUBLIC_URL=http://192.168.1.50:8080`. Because the address never changes, the QR code can be **printed once** and reused every year. Optional: `bingo.local` via mDNS. It works on iOS/macOS but is unreliable on Android, so the QR keeps the IP.
2. **Bind.** `BIND_ADDR=0.0.0.0:8080`, or the specific LAN IP to avoid exposing VPN or virtual adapters.
3. **Firewall.** Allow inbound traffic on the **Private** profile only:
   ```powershell
   New-NetFirewallRule -DisplayName "Christmas Bingo" -Direction Inbound -Protocol TCP -LocalPort 8080 -Action Allow -Profile Private
   ```
   Make sure the home Wi-Fi is set to the *Private* network profile in Windows.
4. **QR generation.** At startup the server prints `PUBLIC_URL` as a terminal QR (`qrcode` crate, Unicode) together with the host PIN. It also serves `GET /qr.svg`, and the host lobby screen shows the QR next to the join code. The QR holds only the URL, never the join code, so the printed QR stays valid across games.
5. **Web build.** `cd app && npx expo export --platform web` creates `app/dist/`. The server serves it via `ServeDir` with an `index.html` fallback for expo-router deep links. Same origin, so **no CORS** in production.
6. **Native (Expo Go / dev builds).** Set `EXPO_PUBLIC_API_BASE_URL=http://192.168.1.50:8080`. CORS is enabled **only** for the origins listed in `DEV_CORS_ORIGINS` (e.g. Metro on `:8081`). Native builds need cleartext HTTP allowed: Android `usesCleartextTraffic` via `expo-build-properties`, iOS `NSAllowsLocalNetworking`. iOS shows a "Local Network" permission prompt the first time.
7. **No HTTPS in v1.** The page is not a secure context, but v1 uses none of the APIs that need one (no service worker, camera or clipboard), and tokens are made on the server.
8. **Network assumptions.** Phones are on the **same Wi-Fi as the host, not a guest network**. Guest networks usually turn on client isolation, which blocks this.
9. **Run.** `server/run.ps1` builds the web app if it's stale, runs `cargo run --release`, and keeps the machine awake during play.

---

## 6) Security baseline (LAN-only)

- **Network:** bound to the LAN, firewall rule on the Private profile only, never port-forwarded. Middleware rejects any peer IP that isn't loopback, RFC 1918, link-local or ULA (defence in depth).
- **Host authority:** a 4–6 digit `HOST_PIN` (from config, or random and printed at startup) is required to create a game. After that, the host token covers start, calls, kick and end.
- **Players:** the join code is 6 chars from an unambiguous alphabet (`ABCDEFGHJKMNPQRSTUVWXYZ23456789`) and is not case-sensitive. Tokens are 128-bit random and only hashes are stored.
- **Rate limiting** (`tower_governor`, per IP): join 10/min, claims 20/min, everything else 120/min. Plus the claim cooldown.
- **Input limits:** request body ≤ 16 KB, request timeout 10 s, display name 1–20 chars after trim, NFC-normalised, control characters stripped. React escapes output, so there is no HTML-injection path.
- **SQL:** SQLx parameterised queries only. Compile-time checked macros with `cargo sqlx prepare` (offline mode).
- **Data minimisation:** display names only, no other personal data. Optionally delete finished games after N days.
- **Accepted risk:** without TLS, anyone on the home Wi-Fi could sniff tokens. That's acceptable for a family game on a trusted network and is documented.

---

## 7) Milestones (small, testable increments)

| # | Milestone | Done when |
|---|---|---|
| **M0** | Scaffold: `server/` (cargo, axum, tracing, config), `app/` (Expo SDK latest, TypeScript, expo-router), `data/` (migrations dir, `.gitignore` for `*.db`). Root README with dev commands. | `GET /api/v1/health` returns ok. Expo Web page shows health status. `cargo test` and `npm test` run. |
| **M1** | Schema migration `0001_init.sql`, DB bootstrap (WAL, FK), card-set manifest parser + validator + importer, `christmas-icons` set. `GET /card-sets[/{slug}]`. | Import is idempotent (second run doesn't change anything). A set with fewer than 24 items is rejected with a clear error. Unit + `sqlx::test` tests pass. |
| **M2** | `rules/` module: seeded card generation, line detection, claim evaluation. | Table-driven tests cover all 12 lines, free centre, the 4 outcome rows, determinism. |
| **M3** | Game lifecycle: create (PIN), current, join, me, start, end, kick. Token hashing, auth extractors, error model. | Integration tests: one-open-game constraint, duplicate name → 409, bad code → 409, wrong token → 401. |
| **M4** | Calls (random / by key) + claims with outcomes in an IMMEDIATE transaction, cooldown, `ALREADY_WON`, `/state` polling. | Integration test plays a whole game: create → 3 join → calls → invalid bingo → cooldown → valid bingo → no_bingo confirmed → end. |
| **M5** | Player app: join screen, card grid (emoji + label tiles), tap to daub, called-item glow, Bingo / No Bingo buttons, result modal (snow burst on a win, gentle "not yet!" otherwise), 2.5 s polling, token storage wrapper, rejoin on reload. | Works on an iPhone (Safari) and an Android (Chrome) phone over Expo Web, and on one device through Expo Go. |
| **M6** | Host app: PIN entry, pick card set, lobby with join code + QR + player list, call controls with called-items board, claims feed, end game. | One person can host a full game from a laptop browser or tablet. |
| **M7** | LAN packaging: web export served by Axum (SPA fallback), terminal QR + `/qr.svg`, firewall script, `run.ps1`, private-IP middleware, rate limits. | A phone scans the printed QR → plays with no laptop help. Works from a cold start of the server. |
| **M8** | Hardening + family dress rehearsal: error UX for every error code, DB backup (`VACUUM INTO` on game end), README "Christmas Eve checklist", authoring guide for card sets. | 6+ devices at once, server restarted mid-game, nobody loses their card (rejoin via stored token). |

---

## 8) Risks and mitigations

| Risk | Mitigation |
|---|---|
| Host IP changes, so the printed QR breaks | DHCP reservation. Server warns at startup if no local interface matches `PUBLIC_URL`. |
| Guest Wi-Fi / AP isolation blocks phones | Checklist item: use the main Wi-Fi. `/health` page to test quickly. |
| Windows firewall or a "Public" network profile blocks access | Firewall script + checklist + startup self-test message |
| Emoji look different per device, or are missing on old Android | Use only Emoji ≤ 11 glyphs, always show the text label, `image` items as a fallback |
| Expo Web and native behave differently (storage, layout) | Storage wrapper, plain RN primitives, test on both in M5 |
| Lost token (cleared browser, new device) | Host can kick the old entry. Rejoin by name is then allowed. (Open question 5) |
| Arguments about a result | Server is the authority and uses the host's call log. Called-items board is visible to everyone. Claims log is stored. |
| SQLite lock contention | WAL, `busy_timeout`, short IMMEDIATE transactions, small pool. Load is about 10 clients polling. |
| Host laptop sleeps mid-game | `run.ps1` keeps the machine awake. Server restart is safe because state lives in SQLite. |
| Copyrighted movie-scene images | Kept in a gitignored `card-sets-private/`. Private household use only. |
| Kids spamming claims | 10 s cooldown after an invalid bingo, friendly messaging, rate limits |

---

## 9) Out of scope for v1

- Printable card generation and PDF layout
- Deeper state analysis or stats dashboards (the schema keeps the raw data for it)
- An **in-app** card-set designer UI (v1 authoring = JSON + asset files + importer validation)
- Several rooms at the same time
- User accounts, passwords, persistent profiles
- WebSockets / SSE / push notifications (polling only)
- Internet or cloud access, HTTPS, remote players
- Pattern variants (blackout, four corners, X) and other grid sizes
- App-store distribution
- Sound effects or background music, i18n

---

## 10) Open questions to answer before coding

1. **"No-bingo" claim:** is "player says they have no bingo; server confirms, or says *you actually do*" the meaning you want? Or did you mean something else, like flagging someone else's claim?
2. **Calling model:** confirmed that the **host calls items** (random draw for icon sets; tap-when-it-happens for movie scenes)? Or should players self-report what they see? Self-reporting can't be checked by the server.
3. **After a win:** keep playing for more winners (proposed), or end the game automatically on the first valid bingo?
4. **Patterns:** lines only with a free centre for v1? OK?
5. **Rejoin policy:** if a player loses their token, should entering the same name + code take back that seat (easy, but anyone could impersonate), or should the host have to kick first (proposed)?
6. **Fixed URL:** can you set a DHCP reservation on your router? Which IP and port should the QR use?
7. **v1 card sets:** ship just `christmas-icons` (emoji), or also a movie-scenes set using your own images in the first demo?
8. **Daubing:** players tap to mark (proposed, more fun), or the board marks itself automatically (easier for the youngest kids)? Could be a per-game toggle.
