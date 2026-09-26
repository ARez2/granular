use super::quad_geometry::{quad_corners, top_left_offset};
use crate::{
    BatchRenderer,
    graphics::{DrawSpace, IntoGpuColor, QuadTex},
    utils::*,
};
use glam::prelude::*;

#[derive(Debug)]
enum DebugDrawCommand {
    Line {
        from: Vec2,
        to: Vec2,
        color: [f32; 4],
        thickness_px: f32,
        layer: i32,
        draw_space: DrawSpace,
    },
    Circle {
        center: Vec2,
        radius: f32,
        thickness_px: f32,
        color: [f32; 4],
        layer: i32,
        draw_space: DrawSpace,
    },
}

pub struct DebugDraw {
    ctx: GeeseContextHandle<Self>,
    commands: Vec<DebugDrawCommand>,
}
impl DebugDraw {
    #[allow(unused)]
    pub fn draw_line<C: IntoGpuColor>(
        &mut self,
        from: Vec2,
        to: Vec2,
        color: C,
        thickness_px: f32,
        layer: i32,
        draw_space: DrawSpace,
    ) {
        self.commands.push(DebugDrawCommand::Line {
            from,
            to,
            color: color.into_gpu_color(),
            thickness_px,
            layer,
            draw_space,
        });
    }

    #[allow(unused)]
    pub fn draw_polyline<C: IntoGpuColor + Clone>(
        &mut self,
        points: &[Vec2],
        color: C,
        thickness_px: f32,
        layer: i32,
        draw_space: DrawSpace,
    ) {
        for pts in points.windows(2) {
            self.commands.push(DebugDrawCommand::Line {
                from: pts[0],
                to: pts[1],
                color: color.clone().into_gpu_color(),
                thickness_px,
                layer,
                draw_space,
            });
        }
    }

    #[allow(unused, clippy::too_many_arguments)]
    pub fn draw_rect<C: IntoGpuColor + Clone>(
        &mut self,
        topleft: Vec2,
        size: Vec2,
        angle_rad: f32,
        color: C,
        thickness_px: f32,
        layer: i32,
        draw_space: DrawSpace,
    ) {
        let center = topleft - top_left_offset(size, draw_space);
        self.draw_rect_center(
            center,
            size,
            angle_rad,
            color,
            thickness_px,
            layer,
            draw_space,
        );
    }

    #[allow(unused, clippy::too_many_arguments)]
    pub fn draw_rect_center<C: IntoGpuColor + Clone>(
        &mut self,
        center: Vec2,
        size: Vec2,
        angle_rad: f32,
        color: C,
        thickness_px: f32,
        layer: i32,
        draw_space: DrawSpace,
    ) {
        let quad_pts = quad_corners(center, size, angle_rad, draw_space);
        let top = draw_space.top_sign();
        // If we would not add the extra offset, the lines produce weird corners like this:
        //   ______
        //   |
        // | x_|___
        // |   |
        // |   |
        let extra = thickness_px / 2.0;
        // TL <-> BL
        self.commands.push(DebugDrawCommand::Line {
            from: quad_pts[0] + vec2(0.0, top * extra),
            to: quad_pts[1] + vec2(0.0, -top * extra),
            color: color.clone().into_gpu_color(),
            thickness_px,
            layer,
            draw_space,
        });
        // TL <-> TR
        self.commands.push(DebugDrawCommand::Line {
            from: quad_pts[0] - vec2(extra, 0.0),
            to: quad_pts[3] + vec2(extra, 0.0),
            color: color.clone().into_gpu_color(),
            thickness_px,
            layer,
            draw_space,
        });
        // BL <-> BR
        self.commands.push(DebugDrawCommand::Line {
            from: quad_pts[1] - vec2(extra, 0.0),
            to: quad_pts[2] + vec2(extra, 0.0),
            color: color.clone().into_gpu_color(),
            thickness_px,
            layer,
            draw_space,
        });
        // BR <-> TR
        self.commands.push(DebugDrawCommand::Line {
            from: quad_pts[2] + vec2(0.0, -top * extra),
            to: quad_pts[3] + vec2(0.0, top * extra),
            color: color.into_gpu_color(),
            thickness_px,
            layer,
            draw_space,
        });
    }

    pub fn draw_circle<C: IntoGpuColor>(
        &mut self,
        center: Vec2,
        radius: f32,
        thickness_px: f32,
        color: C,
        layer: i32,
        draw_space: DrawSpace,
    ) {
        self.commands.push(DebugDrawCommand::Circle {
            center,
            radius,
            thickness_px,
            color: color.into_gpu_color(),
            layer,
            draw_space,
        });
    }

    fn on_record(&mut self, _: &crate::graphics::events::RecordInternalUiRenderingCommands) {
        let mut batch_renderer = self.ctx.get_mut::<BatchRenderer>();

        for cmd in &self.commands {
            match cmd {
                DebugDrawCommand::Line {
                    from,
                    to,
                    color,
                    thickness_px,
                    layer,
                    draw_space,
                } => {
                    let from = *from;
                    let to = *to;

                    let direction = to - from;
                    let length = direction.length();

                    if length <= f32::EPSILON || *thickness_px <= 0.0 {
                        continue;
                    }

                    let center = (from + to) * 0.5;
                    let angle = direction.to_angle();

                    let size = vec2(length, *thickness_px);

                    batch_renderer.draw_quad_with_center(
                        center,
                        size,
                        angle,
                        *color,
                        QuadTex::None,
                        *layer,
                        *draw_space,
                    );
                }
                DebugDrawCommand::Circle {
                    center,
                    radius,
                    thickness_px,
                    color,
                    layer,
                    draw_space,
                } => {
                    batch_renderer.draw_circle(
                        *center,
                        *radius,
                        *thickness_px,
                        *color,
                        *layer,
                        *draw_space,
                    );
                }
            }
        }
        self.commands.clear();
    }
}
impl GeeseSystem for DebugDraw {
    const DEPENDENCIES: Dependencies = dependencies().with::<Mut<BatchRenderer>>();
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers().with(Self::on_record);

    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        Self {
            ctx,
            commands: vec![],
        }
    }
}
