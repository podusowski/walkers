![Hypermocker logo](logo.jpeg)

Hypermocker is a HTTP mocking library the async programming was invented for.

First, Hypermocker will panic, when unanticipated request is made. To anticipate
a request, use `anticipate` function. What is unique to Hypermocker, such
anticipated request, _will not_ be responded immediately. Instead, it will
stay active, until you chose to do so.

```rust
tokio_test::block_on(async {
    // Bind HTTP server to random, free port.
    let server = hypermocker::Server::bind().await;

    // Tell the server not to panic on such request.
    let mut request = server.anticipate("/api/article").await;

    // Do some work, while HTTP request remains in progress.
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Send the response.
    request.respond(b"hello").await;
});
```
