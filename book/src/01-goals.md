# Goals

State the scope and goals of the project.

## Use Cases

- Rust backend with rust frontend(s) within the same Cargo Workspace
- CSS with SCSS
- SPA with no backend
- Assets generation to distribute on CDNs
    * handling hash to invalidate cache
    * utilities for backend development to add these hash to resource links
    * how to do that for links from CSS/JS/whatever (e.g., link to an image from
    CSS). (à la `image-url()` of sprockets?)
- image optimization
- When needed, Handle downloading/execution of external binaries (e.g.,
  `wasm-opt`, `dart-sass`).
- Watch server to re-build in dev as soon as something changes

No goals:

- No JS support at the beginning. It might come later if needed.

## Principles

- No external tools. There should be no need to download and run `npm`.
- Minimal external deps, if a pure-rust solution can be used, we should switch
  to it. As an example, if a viable alternative to dart-sass emerges (e.g.,
  [grass][grass], [rsass][rsass]), we might want to use it instead of using an
  external binary.
- Minimal rust deps, we don't want to clash with the user choices. We should
  stay agnostic as much as possible. For example, that prevents us to use
  `anyhow` for error handling.
- Extensible for custom processing by providing hooks at relevant steps.

## Usage

Pickler is designed to be used as an xtask: an inner crate that takes care of
performing actions. For more information about the xtask principle read [Alex
Kadlov (aka matklad)'s description][1].

Pickler provides the features but by design the user must assemble them.

## Inspiration

Pickler draws inspiration from many different projets and sometimes directly
imports code from them:

- [Trunk][https://trunkrs.dev/], an external packer more focused on SPAs
- [xtask-wasm][https://github.com/rustminded/xtask-wasm/], a direct inspiration
- [Webpack][https://webpack.js.org/], the well known bundler from the JS world
- [Sprocket][https://github.com/rails/sprockets], the rails packaging system


[grass]: https://github.com/connorskees/grass
[rsass]: https://github.com/kaj/rsass
[1]: https://github.com/matklad/cargo-xtask