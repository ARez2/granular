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
- ✅ Done: Make user provide cell logic (easy)
- ✅ Done: Test simulation shader hot reloading
- ✅ Done: Store colors inside cell
- Find way to remove GraphicsSystem dependency from Game?
- Write some usage information (simulation setup + requirements)
- Maybe provide a working webpage with the testbed running?

**Low priority:**
- Dynamically remove textures from `BatchRenderer` texture atlasses
- Input system: What about touch gestures?

