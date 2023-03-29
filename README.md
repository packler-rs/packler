# Packler, bundle Rust for the web

> **Warning**
> This is not ready yet. 


## Usage

A basic xtask main file using Packler:

```rust
use packler::{PacklerConfig, PacklerParams, Run};

fn main() {
    dotenv::from_filename(".env.deploy").ok();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let run = Run::new(
        PacklerParams::new(
            ["app.scss", "admin.scss"],
            [""; 0], // No WASM frontend
            Some("server"),
            Some("prod-pikz-assets"),
        ),
        PacklerConfig::default(),
    );

    run.start();
}
```


## Book

Run the devserver with `$ mdbook serve book/ --open`.

## Other

- [cargo-leptos][leptos], the cargo tools for leptos.
- [trunk][trunk], a tool to build/bundle/ship wasm apps
- [rspack][rspack], a frontend toolchain by Bytedance

[leptos]: https://github.com/leptos-rs/cargo-leptos
[trunk]: https://github.com/thedodd/trunk
[rspack]: https://github.com/web-infra-dev/rspack
