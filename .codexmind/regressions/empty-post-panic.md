# Regression: panic on unknown post ID (2026-10-29)

Symptom: `/r/<sub>/comments/<invalid>.json` returned an empty HTTP reply;
journal showed `cannot access index 0 of JSON array of length 0` at
`serde_json`'s value index.

Cause: `gql::thread` indexed `post["data"]["children"][0]`; GraphQL returns an
empty listing (not an error) for unknown IDs.

Fix: check that the children array is non-empty and return `"Post not found"`;
guarded the context-descent indexing too. Verified: invalid ID -> 404 with
`Error: Post not found`, no panic, service stays up.
