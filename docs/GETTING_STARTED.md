# Prerequisites

* Rust
* Gtk4


# Building prerequisites

```sh
git submodule update --recursive
cd lib/svelte
yarn
yarn link
cd ../..
yarn link svelte
yarn build:esbuild:svelte

# now you can run yarn watch + cargo run
```