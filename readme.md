## Geranium

A simple, fast ATProto image proxy.


### Why geranium?
Geraniums are typically perennials and winter-hardy. They'll grow anywhere that's not waterlogged. Plus, they're pretty.

### Configuration
Geranium is configured via environment variables. None are required:

- `PORT`: The ip:port, or just the port, to listen on. Defaults to 0.0.0.0:3000.
- `CACHE_DIR`: The directory to store cached preprocessed images in. If empty, only memory will be used. Defaults to "./data/foyer".
- `CACHE_SIZE`: The maximum size of the cache, by items. Defaults to 1024.
- `JPEG_QUALITY`: The JPEG quality to use when compressing images. Defaults to 87.
- `ONLY_JPEG`: Will only permit JPEGs from being generated. Defaults to false.
- `MAX_WIDTH`: The maximum width of the image to resize to. Defaults to 1200.
- `MAX_HEIGHT`: The maximum height of the image to resize to. Defaults to 1200.

### Usage
To start the server, run `cargo run`. The server will listen on the port specified by the `PORT` environment variable, or `3000` if not specified.

### Building the Image
docker buildx build --platform linux/amd64,linux/arm64 -t ghcr.io/espeon/geranium/geranium:latest --push .
