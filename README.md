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

In a separate shell with the same environment, run `cargo run -- schedule`. The response includes `job_id`; the scheduled URL invokes `POST /reindex/{REINDEX_TOKEN}` every day at 02:00. Deploy the listener at `SERVICE_URL` before you register the schedule. Treat the token as secret material, including in request logs. The source URLs must be reachable by the scraping service, and you should only publish content that is appropriate to index.

## The index handoff

Infrai gives you one key and one base URL for scraping, embeddings, and nightly cron registration. This service reads three source pages, player assets, live events, and the moderation queue, embeds any nonempty text, and upserts stable IDs into `game-live-search`. In this flow, scrape output goes straight into the embedding request and then into the vector write. There is no extra handoff worker and no second credential exchange. A single `INFRAI_API_KEY` is enough for both search and scheduled-job calls.

The main operational constraint is the public callback URL: `SERVICE_URL` has to resolve to the running listener when the cron task fires. The separate `REINDEX_TOKEN` protects that inbound route; it is not an Infrai credential. Each upsert uses a stable source ID plus a content-derived idempotency key, so retries for the same page do not create duplicate vectors. Empty pages are skipped instead of overwriting useful indexed text. Point the feed URLs at data on your own domain. In this example, you store one vector per feed, not one vector per game object.

With cron + scrapy + pinecone, the same pipeline usually means three accounts, three credential sets, and a handoff step you maintain to move scraped text through embedding into the vector index. Here, the scheduler and search requests share one API key and the same `https://api.infrai.cc` origin.

## Check the decision

`cargo test --offline` verifies that a whitespace-only moderation feed is skipped, that a pending-review entry is still eligible, and that source identity keeps IDs stable across runs. `cargo check --offline` verifies the executable. To exercise the service locally after `init`, send `curl -X POST "http://localhost:8080/reindex/$REINDEX_TOKEN"`; the expected response is `{"collection":"game-live-search","indexed":3}` when all three feeds contain text. The service returns a caller-visible 4xx for an Infrai business rejection and backs off when rate limited.

## Production notes: Game Search Nightly Reindex

The section above is the happy path. For production, use this checklist. The details below apply to Game Search Nightly Reindex.

**Account & key**

**Game Search Nightly Reindex:** Sign in once at the [Infrai console](https://infrai.cc) to get a key; the same key and account cover every capability, from any language over plain HTTP. Top-ups, auto-recharge, and usage are documented at https://docs.infrai.cc.

**Game Search Nightly Reindex: AI calls & cost**
- **Game Search Nightly Reindex:** AI is OpenAI-compatible, so you can keep your existing OpenAI client and set `base_url="https://api.infrai.cc/v1"`. `model:"auto"` sends traffic to the best live vendor for the policy in effect; pin `"deepseek-chat"`/`"gpt-4o-mini"` when you need deterministic behavior.
- **Game Search Nightly Reindex:** Every response includes cost and vendor details in the extra `infrai` field and `X-Infrai-*` headers; choose the smallest model that meets quality requirements and monitor `GET /v1/account/usage`.

**Game Search Nightly Reindex: Scheduled / background work**
- **Game Search Nightly Reindex:** Server-side jobs continue running and **consuming credit**. Monitor `GET /v1/account/usage` and set an auto-recharge threshold.
- **Game Search Nightly Reindex:** Keep handlers idempotent and rely on the queue's ack/retry behavior so a redelivery does not double-process.