# villager-bucket

Carry a villager in a bucket without losing its identity or saved state.

A script for Play Strawberry servers, by Strawberry Interactive LLC. Its texts are in English, French, Spanish and Turkish.

## An example of working with Play Strawberry's tools

Play Strawberry runs a model tool on every server for items with their own look. The tool itself is not part of this
repository, and neither is how it builds the resource pack. This script shows how another script uses it:

- `libs/strawberry-tools` is the public calling side of the tools: its `models` module builds the requests a script
  sends to the model tool (the model it registers, the item it gives to a player, the model id it reads from a stack) and
  its `ui` module the menus and dialogs of the interface tool. It holds no code of either tool.
- `src/models.rs` registers one model when the script loads: a name, its names in four languages, a texture, the base
  item and the stack size. Once the model is registered and in the server's resource pack, `give` and `hand` hand the item
  to a player; `model_id` tells whether a stack is the script's own.
- When the model tool is not on the server, or the model is not in the pack yet, the script falls back to a plain bucket
  item and nothing is lost: every call answers an error kind the caller can act on.

## Install on a Play Strawberry server

Add it to your server in the Play Strawberry panel and restart the server. Play Strawberry servers load only scripts
delivered by Play Strawberry, so a file you build yourself from this source loads only on your own Play Strawberry
developer server.

## Build from source

You need Rust with the `wasm32-wasip2` target and the Strawberry plugin SDK (`strawberry-plugin-api`), which Play
Strawberry gives to its script developers. `Cargo.toml` takes it from the SDK registry named `strawberry`; add that
registry to your Cargo configuration (for example `~/.cargo/config.toml`):

The registry is open and needs no key. Accept the SDK licence on https://playstrawberry.net/docs/get-started/get-the-sdk
before you fetch the SDK.

```toml
[registries.strawberry]
index = "sparse+https://sdk.playstrawberry.net/index/"
```

```text
cargo test
cargo build --release --target wasm32-wasip2
```

The script file is `target/wasm32-wasip2/release/villager_bucket.wasm`.

## License

Source available under the Strawberry Source-Available License 1.0 ([LICENSE](LICENSE)). In short: you may read, build and
change this code for your own use, and run what you build only on Play Strawberry, without making money from it unless we
approve it in writing. The build Play Strawberry offers may be used on any of your Play Strawberry servers, also ones that
earn money. Selling the code or anything built from it is not allowed. Forks are welcome for sending us changes (pull
requests and issues).
