# Custom GPU accelerated falling sand engine

## Running on WASM
For release, omit the `--dev`

```
wasm-pack build examples/testbed --target web --dev
```

Then open the `examples/testbed/static/index.html` with some web server.

Cargo watch command:
```
cargo watch -s "wasm-pack build examples/testbed --target web --dev"
```

## Running natively
```
cargo run -p testbed
```

## Running with profiling
```
cargo run-testbed-trace
```
(Uses an alias defined in `.cargo/config.toml`)


## Todo
- Dynamically remove textures from `BatchRenderer` texture atlasses
- Find way to remove GraphicsSystem dependency from Game?
- ✅ Done: Make user provide cell logic (easy)
- Write some usage information (simulation setup + requirements)
- Maybe provide a working webpage with the testbed running?
- Test simulation shader hot reloading

- Low priority: Input system: What about touch gestures?

