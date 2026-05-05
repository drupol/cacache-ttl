![GitHub stars][GitHub stars]
[![Crates.io Version][Crates.io Version]][cacache-ttl crates]
[![Crates.io License][Crates.io License]][cacache-ttl crates]
[![Donate!][Donate!]][sponsor link]

# Cacache TTL

`cacache-ttl` is a [decorator] for [`cacache`](https://crates.io/crates/cacache)
that adds per-entry [TTL] support while keeping the same read/write style.

The crate stores expiration data in the `cacache` index metadata under
`expires_at_millis`. Cache content is still written and read by `cacache`
itself.

## Usage

Synchronous:

```rust
use std::time::Duration;

fn main() -> cacache::Result<()> {
    let cache = "./target/example-cache";
    let key = "github-response";

    cacache_ttl::write_sync(cache, key, b"cached bytes", Duration::from_secs(300))?;
    let bytes = cacache_ttl::read_sync(cache, key)?;

    assert_eq!(bytes, b"cached bytes");
    Ok(())
}
```

Asynchronous:

```rust
use std::time::Duration;

#[tokio::main]
async fn main() -> cacache::Result<()> {
    let cache = "./target/example-cache";
    let key = "github-response";

    cacache_ttl::write(cache, key, b"cached bytes", Duration::from_secs(300)).await?;
    let bytes = cacache_ttl::read(cache, key).await?;

    assert_eq!(bytes, b"cached bytes");
    Ok(())
}
```

## Expiration Behavior

Expired entries are removed from the `cacache` index with `cacache::remove` /
`cacache::remove_sync`, then returned as `cacache::Error::EntryNotFound`,
matching the miss behavior used by `cacache`.

`cacache::remove` removes the index entry only; the content-addressed bytes may
remain in the cache until normal cache maintenance clears unused content.

Calling `write` or `write_sync` preserves existing JSON metadata and raw
metadata for the key, then adds or updates `expires_at_millis`.

## API

- `read(cache, key)`: async TTL-aware read.
- `read_sync(cache, key)`: synchronous TTL-aware read.
- `list_sync(cache)`: synchronous TTL-aware index listing.
- `write(cache, key, data, ttl)`: async write with TTL.
- `write_sync(cache, key, data, ttl)`: synchronous write with TTL.

[decorator]: https://en.wikipedia.org/wiki/Decorator_pattern
[GitHub stars]: https://img.shields.io/github/stars/drupol/cacache-ttl.svg?style=flat-square
[Donate!]: https://img.shields.io/badge/Sponsor-Github-brightgreen.svg?style=flat-square
[sponsor link]: https://github.com/sponsors/drupol
[Crates.io License]: https://img.shields.io/crates/l/cacache-ttl?style=flat-square
[Crates.io Version]: https://img.shields.io/crates/v/cacache-ttl?style=flat-square
[cacache-ttl crates]: https://crates.io/crates/cacache-ttl
[TTL]: https://en.wikipedia.org/wiki/Time_to_live
