![Hypermocker logo](logo.jpeg)

Hypermocker is a HTTP mocking library the async programming was invented for.

First, Hypermocker will panic, when unanticipated request is made. To anticipate
a request, use `anticipate` function. What is unique to Hypermocker, such
anticipated request, _will not_ be responded immediately. Instead, it will
stay active, until you chose to do so.

```rust
let mut request = server.anticipate("/api/article").await;

// Do something else.

request.respond(Bytes::from_static(b"hello")).await;
```
