# Custom GPU accelerated falling sand engine

## Todo
- Fix `AssetSystem` for WASM
- Fix `BatchRenderer` on WASM (`BINDING_INDEXING` not supported on WASM)
- Integrate `wgpu_profiler` into rendering
- Integrate new compute shader based simulation into granular
- Low priority: Input system: What about touch gestures?


If sending geese events via ctx from inside a future is possible, the following changes to AssetSystem can be made:
- Introduce reference "&" to event in `fn asset_loaded(&mut self, event: &events::AssetLoaded)`
- Register `asset_loaded` as event handler in `impl GeeseSystem for AssetSystem`
- Remove `poll_for_asset_loads` and `check_asset_loads` and the mpsc stuff (also from `queue_load`)