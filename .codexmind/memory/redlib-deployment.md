# Redlib deployment

- Server: `172.16.1.100`, Debian 13 LXC, `redlib.service`, listens `0.0.0.0:8080`.
- Source: `/root/redlib`, branch `main`, remote `mrnamwen/redlib`.
- Binary: `/root/redlib/target/release/redlib`; backups `redlib.bak-*`.
- Env in unit: `REDLIB_PREFERENCES_PATH=/var/lib/redlib/preferences.txt`,
  `REDLIB_JSON_CACHE_PATH=/var/lib/redlib/cache/json`, max age 86400.
- Toolchain installed: rustup 1.99, cmake, clang, ninja, go (BoringSSL builds).
- Deploy: build locally `cargo build --release`, scp to `.new`, stop, mv, start.
- Verify: `curl http://172.16.1.100:8080/r/politics/comments/<id>`.

## 2026-10-29 state

GraphQL backend (`src/gql.rs`, `src/gql_feed.rs`) is the data source; anonymous
JSON is fully blocked. Local test ports used: 8097/8098; spare server port 8096.
