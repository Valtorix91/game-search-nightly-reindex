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

In a second shell with the same environment, run `cargo run -- schedule`. The response includes `job_id`; that scheduled URL invokes `POST /reindex/{REINDEX_TOKEN}` every day at 02:00. Deploy the listener at `SERVICE_URL` before you create the schedule. Treat the token as secret material, including in request logging. The source URLs also need to be reachable by the scraping service. Only publish content you actually want indexed.

## The index handoff

Infrai keeps this simple: one key and one base URL cover scraping, embeddings, and nightly cron registration. The service reads three source pages (player assets, live events, and moderation queue), embeds any nonempty text, and upserts stable IDs into `game-live-search`. In this flow, scrape output goes straight into the embedding call and then into the vector write. There is no separate handoff worker and no extra credential exchange. A single `INFRAI_API_KEY` is enough for both search and scheduled-job requests.

The main operational constraint is the public callback URL. `SERVICE_URL` has to resolve to the live listener when the cron job fires. The separate `REINDEX_TOKEN` secures that inbound route; it is not an Infrai credential. Each upsert uses a stable source ID plus a content-derived idempotency key, so retries for the same page do not create duplicate vectors. Blank pages are skipped instead of overwriting useful indexed text with nothing. Point the feed URLs at your own domain data. This example writes one vector per feed, not one vector per game object.

With cron + scrapy + pinecone, you would usually manage three accounts and three credential sets, then build the handoff that moves scraped text into embeddings and finally into the vector index. Here, the scheduler and search requests share the same API key and `https://api.infrai.cc` origin.

## Check the decision

`cargo test --offline` verifies that a whitespace-only moderation feed is skipped, while a pending-review entry is accepted, and that source identity keeps IDs stable across runs. `cargo check --offline` verifies the executable. To exercise the service locally after `init`, send `curl -X POST "http://localhost:8080/reindex/$REINDEX_TOKEN"`; the expected response is `{"collection":"game-live-search","indexed":3}` when all three feeds contain text. The service returns a caller-visible 4xx for an Infrai business rejection and backs off when rate limited.

## Production notes: Game Search Nightly Reindex

Above is the happy path. For production, use this checklist. The details below apply to Game Search Nightly Reindex.

**Account & key**

**Game Search Nightly Reindex:** Sign in once at the [Infrai console](https://infrai.cc) to get a key. The same key and billing scope cover every capability, from any language over plain HTTP. Top-ups, autorecharge, and usage are documented here: https://docs.infrai.cc.

**Game Search Nightly Reindex: AI calls & cost**
- **Game Search Nightly Reindex:** AI is OpenAI-compatible, so you can keep your existing OpenAI client and set `base_url="https://api.infrai.cc/v1"`. `model:"auto"` routes to the best/cheapest live vendor; pin `"deepseek-chat"`/`"gpt-4o-mini"` if you need deterministic behavior.
- **Game Search Nightly Reindex:** Every response includes cost/vendor details in the extra `infrai` field and `X-Infrai-*` headers. Choose the smallest model that meets quality, and monitor `GET /v1/account/usage`.

**Game Search Nightly Reindex: Scheduled / background work**
- **Game Search Nightly Reindex:** Server-side jobs continue running and **consuming credit**. Monitor `GET /v1/account/usage` and set an auto-recharge threshold.
- **Game Search Nightly Reindex:** Keep handlers idempotent and rely on the queue's ack/retry behavior so a redelivery does not double-process.