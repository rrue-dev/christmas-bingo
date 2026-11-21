# data/

Everything SQLite-related lives here. The server owns the code; this folder owns the schema, the seed content and the database file.

| Path | What it is | In git? |
|---|---|---|
| `migrations/` | SQL migrations, embedded into the server binary (M1) | yes |
| `card-sets/<slug>/set.json` | Card-set manifests and their assets (M1) | yes |
| `card-sets-private/` | Personal card sets, e.g. movie stills for family use only | **no** |
| `bingo.db` | Runtime SQLite database (WAL mode) | **no** |

The server finds this folder through `DATA_DIR` (default `../data`, relative to `server/`).

A card-set authoring guide will be added here in M1, together with the first set, `christmas-icons`.
