# plato-tile-priority

Deadband P0/P1/P2 priority queue - urgency scoring and drain by level

Part of the [PLATO framework](https://github.com/SuperInstance) - 72 crates for deterministic AI knowledge management.

## Tile Pipeline

This crate fits into the PLATO tile lifecycle:

`validate` -> `score` -> `store` -> `search` -> `rank` -> `prompt` -> `inference`

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
plato-tile-priority = "0.1"
```

Zero external dependencies. Works with `cargo 1.75+`.

[GitHub](https://github.com/SuperInstance/plato-tile-priority)
