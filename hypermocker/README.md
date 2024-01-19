![Hypermocker logo](logo.jpeg)

Hypermocker is a HTTP mocking library the async programming was invented for.

```rust
let mut request = server.anticipate("/api/article".to_string()).await;
// Do something else.
request.respond(Bytes::from_static(b"hello")).await;
```
