# Goals

Packler aims to be a self contained toolbox for web development in Rust.
Backend, frontend and assets are covered in this definition.

Packler does not provide an external binary, it is meant to be integrated into
an [xtask][xtask].

## Use Cases

- Rust backend with Rust frontend(s) within the same Cargo Workspace
- CSS with SCSS
- WASM Single page app (SPA) in Rust with no backend
- Assets generation to distribute on CDNs
    * handling hash to invalidate cache
    * utilities for backend development to add these hash to resource links
    * how to do that for links from CSS/JS/whatever (e.g., link to an image from
    CSS). (à la `image-url()` of sprockets?)

## Future 

- Build serverless functions to run in AWS Lambda, Cloudflare, etc. 
- JS/TS support. It might come later if needed (through well known crates like
  [swc][swc])

## Principles

- Compile time as short as possible. In the end, this tool does not bring any business
  value, it should be as transparent as possible.
- As much self contained as possible. There should be no need to download and run `npm`.
- Only when needed, Handle downloading/execution of external binaries (e.g.,
  `wasm-opt`, `dart-sass`).
- Minimal external deps, if a pure-rust solution can be used, we should switch
  to it. As an example, if a viable alternative to dart-sass emerges (e.g.,
  [grass][grass], [rsass][rsass]), we might want to use it instead of using an
  external binary.
- Minimal rust deps, we don't want to clash with the user choices. We should
  stay agnostic as much as possible. For example, that prevents us to use
  `anyhow` for error handling.
- No macros policy (or at least as few as possible) to ensure fast compile time.
- Extensible for custom processing by providing hooks at relevant steps.
- Reasonable logging for debugging

## Usage

Pickler is designed to be used as an xtask: an inner crate that takes care of
performing actions. For more information about the xtask principle read [Alex
Kadlov (aka matklad)'s description][cargo-xtask].

Pickler provides the features but the user must assemble them.

## Inspiration

Pickler draws inspiration from many different projets and sometimes directly
imports code from them:

- [Trunk](https://trunkrs.dev/), an external packer more focused on SPAs
- [xtask-wasm](https://github.com/rustminded/xtask-wasm/), a direct inspiration
- [Webpack](https://webpack.js.org/), the well known bundler from the JS world
- [Sprocket](https://github.com/rails/sprockets), the rails packaging system



[xtask]: https://github.com/matklad/cargo-xtask
[grass]: https://github.com/connorskees/grass
[rsass]: https://github.com/kaj/rsass
[cargo-xtask]: https://github.com/matklad/cargo-xtask
[swc]: https://github.com/swc-project/swc