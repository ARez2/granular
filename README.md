# Custom GPU accelerated falling sand engine

## Running on WASM
For release, omit the `--dev`

```
wasm-pack build examples/testbed --target web --dev
```

Then open the `examples/testbed/static/index.html` with some web server.

## Running natively
```
cargo run -p testbed
```


## Todo
- ✅ Done: Fix `AssetSystem` for WASM
- Fix `BatchRenderer` on WASM (`BINDING_INDEXING` not supported on WASM)
- Integrate new compute shader based simulation into granular
- Integrate `wgpu_profiler` into rendering
- Integrate `include-wgsl-oil` (also for `Vertex`)
- Low priority: Input system: What about touch gestures?

