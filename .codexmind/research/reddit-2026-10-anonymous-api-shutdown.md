# Reddit anonymous API shutdown (2026-10)

## Timeline

- **2026-10-20**: `/r/<sub>/comments/...` and `/comments/...` JSON return 403
  HTML "blocked by network security"; listings still work. Workaround:
  encode the slash (`/comments%2F<id>`), PR redlib-org/redlib#592.
- **2026-10-28/29**: all anonymous JSON routes blocked (`/by_id`, feeds,
  `/api/v1/me`, search). A fresh app-only OAuth token still mints, but every
  `oauth.reddit.com` request returns the same HTML 403. GraphQL
  (`gql-fed.reddit.com`) with a plain token also 403s in ad-hoc curl tests.
- HTML pages now serve a JS challenge (`jsc_token`, doubled-nonce solution).
  Solving it with wreq/browser headers yields real HTML (30 `shreddit-post`
  elements) but does not unlock JSON.
- Public instances in issue #604 were mostly returning 403/418/429.

## Working source

polak14/redlib commit `29f1835` (2026-10-28) routes **everything** through
Android persisted GraphQL operations, converted back to the old JSON shapes.
Built and verified live: listings 200, thread 200 with 200 comments, front,
popular, all, users, search, wiki, duplicates all 200.

## Key implementation points

- `gql.rs`: `PostComments` (hash `314f29c2...`), cursor base64, context
  pruning, RTJSON -> HTML.
- `gql_feed.rs`: `SubredditFeedElements`, `PopularFeedElements`, `AllFeedSdui`,
  `PostsByIds`, `SubredditInfo`, `User`, `Wiki`, `Duplicates`, searches.
- Routing in `client::json`: comments first, then `gql_feed::route`, then legacy
  HTTP only if neither handles the path.
- `x-ratelimit-remaining` is 600/token for GraphQL; rate-limit check expects 599.
- Empty-sort paths `/r/<sub>/.json` must be normalised to `/r/<sub>/hot.json`.

## Other avenues tested

- GraphQL with a hand-minted token: 403 (needs the full Android device header
  set / token binding; the app in Redlib does it correctly).
- Challenge cookies do **not** unlock `.json`.
- `.rss` listings still work (200) but lack scores/media; not used.
- Shreddit comment fragments still work via wreq (200, ~313 comments), retained
  as a fallback.

## Final production state (2026-10-29)

- Anonymous JSON: dead for every IP tested (home + server), with or without a
  freshly minted app-only token, cookies from the HTML challenge, or alternate
  hosts (`api.`, `old.`, `reddit.com`).
- HTML: `www.reddit.com/r/<sub>/` is an 8 KB JS challenge; solving it (doubled
  nonce + `jsc_token`, browser headers, cookie) returns real HTML with
  `shreddit-post` elements, but does not unlock JSON. Not used by Redlib.
- Shreddit comment fragments still return 200 via wreq; retained only as the
  comment-page fallback.
- GraphQL (`gql-fed.reddit.com`) with the full Android device headers works and
  is now the primary backend.
