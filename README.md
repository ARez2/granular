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
- ✅ Done: Fix `AssetSystem` for WASM
- ✅ Done: Fix `BatchRenderer` on WASM (`BINDING_INDEXING` not supported on WASM)
- Dynamically remove textures from `BatchRenderer` texture atlasses
- Integrate new compute shader based simulation into granular
- Integrate `wgpu_profiler` into rendering
- Integrate `include-wgsl-oil` (also for `Vertex`)
- Low priority: Input system: What about touch gestures?

