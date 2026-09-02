//! Bounded media reference and local acquisition contract tests.

use std::time::Duration;

use qq_media::{
    MediaPolicy, MediaReference, MediaResolver, MediaSourceKind, RemoteMediaPolicy,
};

#[tokio::test]
async fn local_base64_and_cache_share_one_size_policy() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let local = temporary.path().join("input.bin");
    tokio::fs::write(&local, b"media").await?;
    let cache = temporary.path().join("cache");
    tokio::fs::create_dir(&cache).await?;
    let key = "1".repeat(64);
    tokio::fs::write(cache.join(&key), b"cached").await?;
    let resolver = MediaResolver::new(MediaPolicy::new(
        vec![temporary.path().to_owned()],
        Some(cache),
        16,
        None,
    )?);

    let local_object = resolver.resolve(&MediaReference::Local(local)).await?;
    assert_eq!(local_object.bytes(), b"media");
    assert_eq!(local_object.source_kind(), MediaSourceKind::LocalFile);
    let inline = resolver
        .resolve(&MediaReference::parse("base64://bWVkaWE=")?)
        .await?;
    assert_eq!(inline.bytes(), b"media");
    let cached = resolver
        .resolve(&MediaReference::parse(&format!("cache://{key}"))?)
        .await?;
    assert_eq!(cached.bytes(), b"cached");
    Ok(())
}

#[test]
fn remote_media_supports_normal_web_urls_and_optional_policy()
-> Result<(), Box<dyn std::error::Error>> {
    assert!(MediaReference::parse("http://example.com/a").is_ok());
    assert!(MediaReference::parse("https://example.com/a").is_ok());
    assert!(RemoteMediaPolicy::public_web(Duration::ZERO).is_err());
    assert!(
        RemoteMediaPolicy::restricted_web([String::from("EXAMPLE.com")], Duration::from_secs(5))
            .is_err()
    );
    let policy = RemoteMediaPolicy::restricted_web(
        [String::from("media.example.com")],
        Duration::from_secs(5),
    )?;
    assert!(MediaPolicy::new(Vec::new(), None, 1_024, Some(policy)).is_ok());
    let public = RemoteMediaPolicy::public_web(Duration::from_secs(5))?
        .with_denied_hosts([String::from("blocked.example.com")])?;
    assert!(MediaPolicy::new(Vec::new(), None, 1_024, Some(public)).is_ok());
    Ok(())
}
