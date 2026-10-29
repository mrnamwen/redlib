# Regression: anonymous JSON 403 (2026-10-28)

Symptom: all Redlib pages fail; logs show
`Reddit blocked this request with its network security page | /by_id/...`,
listings and threads 404/error.

Cause: Reddit stopped serving its JSON API to app-only anonymous tokens.

Fix: `client::json` routes comments to `gql::thread` and everything else to
`gql_feed::route`, converting GraphQL back to the JSON API shapes.

Guard: `gql_feed::route` normalises `/r/<sub>/.json` (empty sort) to `hot`;
GraphQL rate limit is 600, so the startup check expects 599.
