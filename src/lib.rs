//! TTL-aware decorator for [`cacache`].
//!
//! `cacache-ttl` keeps the familiar `cacache` read/write API shape while adding
//! a `Duration` parameter to writes. Expiration data is stored in the `cacache`
//! index metadata under `expires_at_millis`; cache content is still written and
//! read by `cacache` itself.
//!
//! Expired entries are removed from the `cacache` index with
//! [`cacache::remove`] / [`cacache::remove_sync`] and then surfaced as
//! [`Error::EntryNotFoundOrExpired`], making cache misses and expired entries
//! explicit while still forwarding other `cacache` errors.
//!
//! # Examples
//!
//! Asynchronous usage:
//!
//! ```no_run
//! use std::time::Duration;
//!
//! # async fn example() -> cacache_ttl::Result<()> {
//! cacache_ttl::write("./target/example-cache", "key", b"value", Duration::from_secs(60)).await?;
//! let value = cacache_ttl::read("./target/example-cache", "key").await?;
//! assert_eq!(value, b"value");
//! # Ok(())
//! # }
//! ```
//!
//! Synchronous usage:
//!
//! ```no_run
//! use std::time::Duration;
//!
//! cacache_ttl::write_sync("./target/example-cache", "key", b"value", Duration::from_secs(60))?;
//! let value = cacache_ttl::read_sync("./target/example-cache", "key")?;
//! assert_eq!(value, b"value");
//! # Ok::<(), cacache_ttl::Error>(())
//! ```

use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use cacache::{Integrity, Metadata, Value};

mod error;
pub use error::{Error, Result};

const EXPIRES_AT_MILLIS: &str = "expires_at_millis";

/// Reads a cache entry asynchronously and validates its TTL metadata.
///
/// This mirrors [`cacache::read`] but removes entries with expired
/// `expires_at_millis` metadata from the `cacache` index and treats them as
/// cache misses.
///
/// # Errors
///
/// Returns [`Error::EntryNotFoundOrExpired`] when the entry does not exist or
/// when it has expired. Other errors, including removal errors for expired
/// entries, are forwarded from `cacache` as [`Error::Cacache`].
///
/// # Examples
///
/// ```no_run
/// # use std::time::Duration;
/// # async fn example() -> cacache_ttl::Result<()> {
/// cacache_ttl::write("./target/example-cache", "key", b"value", Duration::from_secs(60)).await?;
/// let value = cacache_ttl::read("./target/example-cache", "key").await?;
/// assert_eq!(value, b"value");
/// # Ok(())
/// # }
/// ```
pub async fn read<P, K>(cache: P, key: K) -> Result<Vec<u8>>
where
    P: AsRef<Path>,
    K: AsRef<str>,
{
    let cache = cache.as_ref();
    let key = key.as_ref();
    let Some(metadata) = cacache::metadata(cache, key).await? else {
        return Err(entry_not_found(cache, key));
    };

    if is_expired(&metadata) {
        cacache::remove(cache, key).await?;
        return Err(entry_not_found(cache, key));
    }

    Ok(cacache::read(cache, key).await?)
}

/// Reads a cache entry synchronously and validates its TTL metadata.
///
/// This mirrors [`cacache::read_sync`] but removes entries with expired
/// `expires_at_millis` metadata from the `cacache` index and treats them as
/// cache misses.
///
/// # Errors
///
/// Returns [`Error::EntryNotFoundOrExpired`] when the entry does not exist or
/// when it has expired. Other errors, including removal errors for expired
/// entries, are forwarded from `cacache` as [`Error::Cacache`].
///
/// # Examples
///
/// ```no_run
/// # use std::time::Duration;
/// cacache_ttl::write_sync("./target/example-cache", "key", b"value", Duration::from_secs(60))?;
/// let value = cacache_ttl::read_sync("./target/example-cache", "key")?;
/// assert_eq!(value, b"value");
/// # Ok::<(), cacache_ttl::Error>(())
/// ```
pub fn read_sync<P, K>(cache: P, key: K) -> Result<Vec<u8>>
where
    P: AsRef<Path>,
    K: AsRef<str>,
{
    let cache = cache.as_ref();
    let key = key.as_ref();
    let Some(metadata) = cacache::metadata_sync(cache, key)? else {
        return Err(entry_not_found(cache, key));
    };

    if is_expired(&metadata) {
        cacache::remove_sync(cache, key)?;
        return Err(entry_not_found(cache, key));
    }

    Ok(cacache::read_sync(cache, key)?)
}

/// Lists non-expired cache index entries synchronously.
///
/// This mirrors [`cacache::list_sync`] but filters out entries with expired
/// `expires_at_millis` metadata. Expired entries are removed from the
/// `cacache` index with [`cacache::remove_sync`] and are not yielded.
///
/// Entries without TTL metadata are yielded unchanged.
///
/// # Errors
///
/// Iterator items forward errors from `cacache` listing and removal as
/// [`Error::Cacache`].
///
/// # Examples
///
/// ```no_run
/// # use std::time::Duration;
/// cacache_ttl::write_sync("./target/example-cache", "key", b"value", Duration::from_secs(60))?;
/// let keys = cacache_ttl::list_sync("./target/example-cache")
///     .map(|entry| entry.map(|entry| entry.key))
///     .collect::<cacache_ttl::Result<Vec<_>>>()?;
/// assert!(keys.contains(&"key".to_owned()));
/// # Ok::<(), cacache_ttl::Error>(())
/// ```
pub fn list_sync<P>(cache: P) -> impl Iterator<Item = Result<Metadata>>
where
    P: AsRef<Path>,
{
    let cache = cache.as_ref().to_path_buf();
    cacache::list_sync(cache.clone()).filter_map(move |entry| match entry {
        Ok(metadata) if is_expired(&metadata) => {
            match cacache::remove_sync(&cache, &metadata.key) {
                Ok(()) => None,
                Err(error) => Some(Err(error.into())),
            }
        }
        other => Some(other.map_err(Into::into)),
    })
}

/// Writes a cache entry asynchronously with a TTL.
///
/// This mirrors [`cacache::write`] with one extra `ttl` parameter. The content
/// is written through `cacache::write`; the expiration timestamp is then stored
/// in index metadata as `expires_at_millis`.
///
/// Existing JSON metadata and raw metadata for the key are preserved when the
/// TTL metadata is added or updated.
///
/// # Errors
///
/// Forwards errors from `cacache` content writes and index updates as
/// [`Error::Cacache`].
///
/// # Examples
///
/// ```no_run
/// # use std::time::Duration;
/// # async fn example() -> cacache_ttl::Result<()> {
/// let integrity =
///     cacache_ttl::write("./target/example-cache", "key", b"value", Duration::from_secs(60))
///         .await?;
/// println!("{integrity}");
/// # Ok(())
/// # }
/// ```
pub async fn write<P, D, K>(cache: P, key: K, data: D, ttl: Duration) -> Result<Integrity>
where
    P: AsRef<Path>,
    D: AsRef<[u8]>,
    K: AsRef<str>,
{
    let cache = cache.as_ref();
    let key = key.as_ref();
    let previous = cacache::metadata(cache, key).await?;
    let integrity = cacache::write(cache, key, data).await?;
    let current = cacache::metadata(cache, key)
        .await?
        .ok_or_else(|| entry_not_found(cache, key))?;
    cacache::index::insert_async(cache, key, write_opts(&current, ttl, previous.as_ref())).await?;

    Ok(integrity)
}

/// Writes a cache entry synchronously with a TTL.
///
/// This mirrors [`cacache::write_sync`] with one extra `ttl` parameter. The
/// content is written through `cacache::write_sync`; the expiration timestamp
/// is then stored in index metadata as `expires_at_millis`.
///
/// Existing JSON metadata and raw metadata for the key are preserved when the
/// TTL metadata is added or updated.
///
/// # Errors
///
/// Forwards errors from `cacache` content writes and index updates as
/// [`Error::Cacache`].
///
/// # Examples
///
/// ```no_run
/// # use std::time::Duration;
/// let integrity =
///     cacache_ttl::write_sync("./target/example-cache", "key", b"value", Duration::from_secs(60))?;
/// println!("{integrity}");
/// # Ok::<(), cacache_ttl::Error>(())
/// ```
pub fn write_sync<P, D, K>(cache: P, key: K, data: D, ttl: Duration) -> Result<Integrity>
where
    P: AsRef<Path>,
    D: AsRef<[u8]>,
    K: AsRef<str>,
{
    let cache = cache.as_ref();
    let key = key.as_ref();
    let previous = cacache::metadata_sync(cache, key)?;
    let integrity = cacache::write_sync(cache, key, data)?;
    let current = cacache::metadata_sync(cache, key)?.ok_or_else(|| entry_not_found(cache, key))?;
    cacache::index::insert(cache, key, write_opts(&current, ttl, previous.as_ref()))?;

    Ok(integrity)
}

fn entry_not_found(cache: &Path, key: &str) -> Error {
    Error::EntryNotFoundOrExpired(cache.to_path_buf(), key.to_owned())
}

fn is_expired(metadata: &Metadata) -> bool {
    let Some(expires_at) = metadata
        .metadata
        .get(EXPIRES_AT_MILLIS)
        .and_then(Value::as_u64)
    else {
        return false;
    };

    now_millis() > expires_at
}

fn expires_at_millis(ttl: Duration) -> u64 {
    now_millis().saturating_add(duration_millis(ttl))
}

fn write_opts(
    current: &Metadata,
    ttl: Duration,
    existing: Option<&Metadata>,
) -> cacache::WriteOpts {
    let opts = cacache::WriteOpts::new()
        .integrity(current.integrity.clone())
        .size(current.size)
        .metadata(metadata_with_ttl(existing, ttl));

    if let Some(raw_metadata) = existing.and_then(|metadata| metadata.raw_metadata.clone()) {
        opts.raw_metadata(raw_metadata)
    } else {
        opts
    }
}

fn metadata_with_ttl(existing: Option<&Metadata>, ttl: Duration) -> Value {
    let mut metadata = existing
        .map(|metadata| metadata.metadata.clone())
        .unwrap_or_else(empty_metadata);

    if !metadata.is_object() {
        metadata = empty_metadata();
    }

    if let Some(object) = metadata.as_object_mut() {
        object.insert(
            EXPIRES_AT_MILLIS.to_owned(),
            Value::from(expires_at_millis(ttl)),
        );
    }

    metadata
}

fn empty_metadata() -> Value {
    Value::Object(Default::default())
}

fn now_millis() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    u64::try_from(now).unwrap_or(u64::MAX)
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use cacache::Value;

    fn object(entries: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
        let mut value = Value::Object(Default::default());
        let object = value.as_object_mut().expect("value is an object");
        for (key, item) in entries {
            object.insert(key.to_owned(), item);
        }
        value
    }

    fn temp_cache_root(name: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();

        std::env::temp_dir().join(format!(
            "cacache-ttl-test-{name}-{}-{now}",
            std::process::id()
        ))
    }

    #[test]
    fn ttl_read_ignores_expired_cache_entries() {
        let root = temp_cache_root("expired");
        let key = "expired-entry";
        let contents = br#"{"ok":true}"#;
        let integrity =
            cacache::write_hash_sync(&root, contents).expect("cache content write succeeds");
        cacache::index::insert(
            &root,
            key,
            cacache::WriteOpts::new()
                .integrity(integrity)
                .size(contents.len())
                .metadata(object([("expires_at_millis", Value::from(1_u64))])),
        )
        .expect("cache index insert succeeds");

        let error =
            crate::read_sync(&root, key).expect_err("expired entry should be treated as missing");
        assert!(matches!(error, crate::Error::EntryNotFoundOrExpired(_, _)));
        assert!(
            cacache::metadata_sync(&root, key)
                .expect("metadata read succeeds")
                .is_none()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn async_ttl_read_removes_expired_cache_entries() {
        let root = temp_cache_root("async-expired");
        let key = "async-expired-entry";
        let contents = br#"{"ok":true}"#;
        let integrity =
            cacache::write_hash_sync(&root, contents).expect("cache content write succeeds");
        cacache::index::insert(
            &root,
            key,
            cacache::WriteOpts::new()
                .integrity(integrity)
                .size(contents.len())
                .metadata(object([("expires_at_millis", Value::from(1_u64))])),
        )
        .expect("cache index insert succeeds");

        let error = crate::read(&root, key)
            .await
            .expect_err("expired entry should be treated as missing");
        assert!(matches!(error, crate::Error::EntryNotFoundOrExpired(_, _)));
        assert!(
            cacache::metadata(&root, key)
                .await
                .expect("metadata read succeeds")
                .is_none()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ttl_read_returns_fresh_cache_entries() {
        let root = temp_cache_root("fresh");
        let key = "fresh-entry";
        let contents = br#"{"ok":true}"#;
        crate::write_sync(&root, key, contents, Duration::from_secs(60))
            .expect("cache write succeeds");

        let cached = crate::read_sync(&root, key).expect("ttl cache read succeeds");
        assert_eq!(cached, contents);

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn async_ttl_read_returns_fresh_cache_entries() {
        let root = temp_cache_root("async-fresh");
        let key = "async-fresh-entry";
        let contents = br#"{"ok":true}"#;
        crate::write(&root, key, contents, Duration::from_secs(60))
            .await
            .expect("cache write succeeds");

        let cached = crate::read(&root, key)
            .await
            .expect("ttl cache read succeeds");
        assert_eq!(cached, contents);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ttl_write_preserves_existing_index_metadata() {
        let root = temp_cache_root("metadata");
        let key = "metadata-entry";
        let original = br#"{"old":true}"#;
        let updated = br#"{"new":true}"#;
        let integrity =
            cacache::write_hash_sync(&root, original).expect("cache content write succeeds");
        cacache::index::insert(
            &root,
            key,
            cacache::WriteOpts::new()
                .integrity(integrity)
                .size(original.len())
                .metadata(object([
                    ("custom", Value::from("preserved")),
                    ("nested", object([("ok", Value::from(true))])),
                ]))
                .raw_metadata(b"raw".to_vec()),
        )
        .expect("cache index insert succeeds");

        crate::write_sync(&root, key, updated, Duration::from_secs(60))
            .expect("cache write succeeds");

        let metadata = cacache::metadata_sync(&root, key)
            .expect("metadata read succeeds")
            .expect("metadata exists");
        assert_eq!(metadata.metadata["custom"], "preserved");
        assert_eq!(
            metadata.metadata["nested"],
            object([("ok", Value::from(true))])
        );
        assert!(metadata.metadata["expires_at_millis"].is_u64());
        assert_eq!(metadata.raw_metadata, Some(b"raw".to_vec()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_sync_yields_only_non_expired_entries() {
        let root = temp_cache_root("list");
        crate::write_sync(&root, "fresh", b"fresh", Duration::from_secs(60))
            .expect("fresh entry write succeeds");

        let expired_integrity =
            cacache::write_hash_sync(&root, b"expired").expect("expired content write succeeds");
        cacache::index::insert(
            &root,
            "expired",
            cacache::WriteOpts::new()
                .integrity(expired_integrity)
                .size(b"expired".len())
                .metadata(object([("expires_at_millis", Value::from(1_u64))])),
        )
        .expect("expired index insert succeeds");

        let regular_integrity =
            cacache::write_hash_sync(&root, b"regular").expect("regular content write succeeds");
        cacache::index::insert(
            &root,
            "regular",
            cacache::WriteOpts::new()
                .integrity(regular_integrity)
                .size(b"regular".len()),
        )
        .expect("regular index insert succeeds");

        let mut keys = crate::list_sync(&root)
            .map(|entry| entry.map(|entry| entry.key))
            .collect::<crate::Result<Vec<_>>>()
            .expect("cache list succeeds");
        keys.sort();

        assert_eq!(keys, vec!["fresh".to_owned(), "regular".to_owned()]);
        assert!(
            cacache::metadata_sync(&root, "expired")
                .expect("metadata read succeeds")
                .is_none()
        );

        let _ = fs::remove_dir_all(root);
    }
}
