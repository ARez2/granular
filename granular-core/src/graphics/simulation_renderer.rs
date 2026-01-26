use glam::IVec2;
use palette::{Srgba, WithAlpha};
use wgpu::{
    Extent3d, ImageDataLayout, SamplerDescriptor, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages, TextureViewDescriptor,
};

use super::{GraphicsSystem, TextureBundle};
use crate::{
    assets::{AssetHandle, TextureAsset},
    chunk::{CHUNK_SIZE, NUM_CELLS_IN_CHUNK},
    graphics,
    utils::*,
    AssetSystem, BatchRenderer, Camera, Simulation, NUM_CHUNKS_TOTAL,
};

#[derive(Debug)]
pub struct SimulationRenderer {
    ctx: GeeseContextHandle<Self>,

    display_scale: i32,
    // Each element here corresponds to the element with the same index
    // in the Simulation.chunks array (both arrays have same length)
    chunk_textures: [AssetHandle<TextureAsset>; NUM_CHUNKS_TOTAL],
}
impl SimulationRenderer {
    pub fn on_draw(&mut self, _: &crate::events::Draw) {
        #[cfg(feature = "trace")]
        let _span = info_span!("SimulationRenderer::on_draw").entered();

        let graphics_sys = self.ctx.get::<GraphicsSystem>();
        let asset_sys = self.ctx.get::<AssetSystem>();
        let sim = self.ctx.get::<Simulation>();
        #[cfg(feature = "trace")]
        let span_write_chunks = info_span!("SimulationRenderer write chunk textures");
        #[cfg(feature = "trace")]
        let span_guard = span_write_chunks.enter();
        // Collect the chunk positions here, because we need to borrow the BatchRenderer
        // mutably later and we cant have Simulation borrowed at the same time
        let mut chunk_positions = vec![IVec2::ZERO; NUM_CHUNKS_TOTAL];
        let mut should_chunk_update = vec![];
        for (idx, chunk) in sim.get_chunks().iter().enumerate() {
            // Fetch the color data from the chunk itself
            let tex_data = chunk.get_texture_data();
            // Get the texture for this chunk from ourself
            let sim_texture = asset_sys
                .get::<TextureAsset>(&self.chunk_textures[idx])
                .texture();
            graphics_sys.queue().write_texture(
                sim_texture.texture().as_image_copy(),
                tex_data,
                sim_texture.data_layout(),
                sim_texture.extent(),
            );
            chunk_positions[idx] = chunk.position;
            if Simulation::DEBUG_UPDATE {
                should_chunk_update.push(chunk.should_update(((sim.tick / 16) % 4) as u8))
            };
        }
        drop(graphics_sys);
        drop(asset_sys);
        drop(sim);
        drop(span_guard);

        let mut quad_renderer = self.ctx.get_mut::<BatchRenderer>();
        for (idx, chunk_pos) in chunk_positions.into_iter().enumerate() {
            let chunk_center =
                chunk_pos * IVec2::new(1, -1) * CHUNK_SIZE as i32 * self.display_scale * 3;
            let chunk_display_size = IVec2::new(
                CHUNK_SIZE as i32 * self.display_scale,
                CHUNK_SIZE as i32 * self.display_scale,
            );
            quad_renderer.draw_quad(
                &graphics::Quad {
                    center: chunk_center,
                    size: chunk_display_size,
                    color: Srgba::from_format(palette::named::WHITE.with_alpha(1.0)),
                    texture: Some(self.chunk_textures[idx].clone()),
                },
                0,
            );
            if Simulation::DEBUG_UPDATE && should_chunk_update[idx] {
                quad_renderer.draw_quad(
                    &graphics::Quad {
                        center: chunk_center,
                        size: chunk_display_size,
                        color: Srgba::from_format(palette::named::RED.with_alpha(0.4)),
                        texture: None,
                    },
                    1,
                );
            }
        }
    }
}
impl GeeseSystem for SimulationRenderer {
    const DEPENDENCIES: geese::Dependencies = dependencies()
        .with::<GraphicsSystem>()
        .with::<Mut<AssetSystem>>()
        .with::<Mut<BatchRenderer>>()
        .with::<Camera>()
        .with::<Simulation>();

    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers().with(Self::on_draw);

    fn new(mut ctx: geese::GeeseContextHandle<Self>) -> Self {
        #[cfg(feature = "trace")]
        let _span = info_span!("SimulationRenderer::new").entered();

        let chunk_textures: [AssetHandle<TextureAsset>; NUM_CHUNKS_TOTAL] =
            core::array::from_fn(|i| {
                let tex_extent = Extent3d {
                    width: CHUNK_SIZE as u32,
                    height: CHUNK_SIZE as u32,
                    depth_or_array_layers: 1,
                };
                let chunk_tex_data = [0u8; NUM_CELLS_IN_CHUNK * 4];
                let graphics_sys = ctx.get::<GraphicsSystem>();
                let device = graphics_sys.device();
                let chunk_texture = TextureBundle::new(
                    device,
                    graphics_sys.queue(),
                    &format!("SimulationRenderer Chunk {} bundle", i),
                    tex_extent,
                    TextureDescriptor {
                        label: Some(&format!(
                            "SimulationRenderer Chunk {} texture descriptor",
                            i
                        )),
                        size: tex_extent,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: TextureDimension::D2,
                        format: TextureFormat::Rgba8UnormSrgb,
                        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                        view_formats: &[],
                    },
                    &TextureViewDescriptor::default(),
                    &SamplerDescriptor {
                        address_mode_u: wgpu::AddressMode::ClampToEdge,
                        address_mode_v: wgpu::AddressMode::ClampToEdge,
                        address_mode_w: wgpu::AddressMode::ClampToEdge,
                        mag_filter: wgpu::FilterMode::Nearest,
                        min_filter: wgpu::FilterMode::Nearest,
                        mipmap_filter: wgpu::FilterMode::Nearest,
                        ..Default::default()
                    },
                    &chunk_tex_data,
                    ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * tex_extent.width),
                        rows_per_image: Some(tex_extent.height),
                    },
                );
                drop(graphics_sys);
                let mut asset_sys = ctx.get_mut::<AssetSystem>();
                asset_sys.register::<TextureAsset>(TextureAsset::from(chunk_texture))
            });

        Self {
            ctx,

            display_scale: 1,
            chunk_textures,
        }
    }
}
