# Villager Bucket

Carry a villager in a bucket without losing it: its profession, level, experience, trades, gossip,
type, age and name stay with it.

**Status: not active yet.** The plugin waits for a server update that lets it save a whole
villager and remove it safely. Until then it loads, says in the console what it is waiting for, and
changes nothing in the game.

## How it will play

- Right-click a villager with an empty bucket, in either hand, to pick it up.
- Right-click a block with the filled bucket to place the villager next to that face; you get the
  empty bucket back.
- A full inventory refuses the pickup before the villager is touched.
- Only villagers: not wandering traders or zombie villagers.
- Normal protection plugins can refuse a pickup or a placement.

Known limits:

- In creative mode a filled bucket can be copied, and each copy places a villager.
- A server crash between taking the villager and giving the bucket can lose that villager.

## Commands, permission and settings

- Commands: none.
- Permission: `villager-bucket:use`, for everyone by default, to pick up and to place. It is
  registered once the plugin is active.
- Settings: none.
- Needs the Play Strawberry custom models plugin on the server, loaded first; it shows the filled
  bucket with its own texture (`models/villager_bucket.png`). When the model cannot be shown, a
  normal bucket with a name and lore is used.

## Languages

English, French, Spanish and Turkish, chosen by each player's game language (`lang/`).

## Install on a Play Strawberry server

1. Add Villager Bucket to your server in the Play Strawberry panel, together with the custom models
   plugin.
2. Restart the server.

Play Strawberry servers load only plugins signed by Play Strawberry, so a file you build yourself
from this source does not load on them.

## Build from source

You need Rust with the `wasm32-wasip2` target and the Strawberry plugin SDK
(`strawberry-plugin-api`), which Play Strawberry gives to its plugin developers.
`Cargo.toml` takes it from the SDK registry named `strawberry`; add that registry to
your Cargo configuration (for example `~/.cargo/config.toml`):

```toml
[registries.strawberry]
index = "sparse+https://sdk.playstrawberry.net/index/"
```

The `custom-models-api` library is in `libs/custom-models-api`; `cargo test --workspace` runs its tests too.

```text
cargo test --workspace
cargo build --release --target wasm32-wasip2
```

The plugin file is `target/wasm32-wasip2/release/villager_bucket.wasm`.

## License

Copyright Strawberry Interactive LLC. The license text is not published yet; until it is, all
rights are reserved.
