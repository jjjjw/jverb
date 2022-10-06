# jverb

## Building

After installing [Rust](https://rustup.rs/), you can compile jverb as follows:

```shell
cargo xtask bundle jverb --release
```

For mac OS M1, you need to build a universal binary for the plugin to work with Ableton
```shell
cargo xtask bundle-universal jverb --release
```

Move to default VST3 folder:
```
sudo mv target/bundled/jverb.vst3 /Library/Audio/Plug-Ins/VST3
```