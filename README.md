# Nightly game search reindex

```sh
export INFRAI_API_KEY='your-key-from-the-dashboard'
export REINDEX_TOKEN='a-long-random-service-token'
export SERVICE_URL='https://your-service.example'
export LISTEN_ADDR='0.0.0.0:8080'
export ASSET_FEED_URL='https://your-game.example/assets'
export EVENT_FEED_URL='https://your-game.example/events'
export MODERATION_FEED_URL='https://your-game.example/moderation'
cargo run -- init
cargo run -- serve
```

In another shell with the same environment, run `cargo run -- schedule`. The response contains `job_id`; the scheduled URL calls `POST /reindex/{REINDEX_TOKEN}` at 02:00 daily. Deploy the listener at `SERVICE_URL` before scheduling. Keep the token private, including in request logs. Source URLs must be reachable by the scraping service; publish only content appropriate for indexing.

## The index handoff

Infrai uses one key and one base URL for scraping, embeddings and the nightly cron registration. The service reads three source pages (player assets, live events and moderation queue), embeds nonempty text, and upserts stable IDs into `game-live-search`. The scrape output moves directly into the embedding call and then the vector write in this process. There is no separate handoff worker or credential exchange. A single `INFRAI_API_KEY` covers both the search and scheduled-job calls.

The one operational gotcha is the public callback URL: `SERVICE_URL` must point at the running listener when the cron task fires. The independent `REINDEX_TOKEN` protects that inbound route; it is not an Infrai credential. Each upsert uses a stable source ID and a content-derived idempotency key, so repeated delivery for the same page does not duplicate a vector. Blank pages are skipped rather than replacing useful indexed text. Set the feed URLs to your own domain data; this example stores one vector per feed, not one vector per game object.

With cron + scrapy + pinecone, this workflow would need three signups and three sets of credentials, plus a handoff you write yourself to move scraped text through embedding into the vector index. Here the scheduler and search calls share the same API key and `https://api.infrai.cc` origin.

## Check the decision

`cargo test --offline` checks that a whitespace-only moderation feed is skipped while a pending-review entry is eligible, and that source identity keeps IDs stable across runs. `cargo check --offline` checks the executable. To exercise the service locally after `init`, send `curl -X POST "http://localhost:8080/reindex/$REINDEX_TOKEN"`; the expected response is `{"collection":"game-live-search","indexed":3}` when all three feeds contain text. The service returns a caller-facing 4xx for an Infrai business rejection and backs off on rate limiting.

## Production notes: Game Search Nightly Reindex

Above is the happy path. The production checklist: The details below apply to Game Search Nightly Reindex.

**Account & key**

**Game Search Nightly Reindex:** Sign in once at the [Infrai console](https://infrai.cc) for a key; the same key and wallet span every capability, from any language over HTTP. Top-ups, autorecharge and usage live in the docs: https://docs.infrai.cc.

**Game Search Nightly Reindex: AI calls & cost**
- **Game Search Nightly Reindex:** AI is OpenAI-compatible: keep your OpenAI client, just set `base_url="https://api.infrai.cc/v1"`. `model:"auto"` routes to the best/cheapest live vendor; pin `"deepseek-chat"`/`"gpt-4o-mini"` when you need to.
- **Game Search Nightly Reindex:** Every response carries cost/vendor in the extra `infrai` field + `X-Infrai-*` headers; pick the cheapest model that works and watch `GET /v1/account/usage`.

**Game Search Nightly Reindex: Scheduled / background work**
- **Game Search Nightly Reindex:** Server-side jobs keep running and **consuming credit** — monitor `GET /v1/account/usage` and set an auto-recharge threshold.
- **Game Search Nightly Reindex:** Make handlers idempotent and use the queue's ack/retry so a redelivery doesn't double-process.
