use crate::{
    BatchRenderer,
    graphics::{DrawSpace, IntoGpuColor},
    utils::*,
};
use glam::prelude::*;

#[derive(Debug)]
enum DebugDrawCommand {
    Line {
        from: IVec2,
        to: IVec2,
        color: [f32; 4],
        thickness: i32,
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
        from: IVec2,
        to: IVec2,
        color: C,
        thickness: i32,
        layer: i32,
        draw_space: DrawSpace,
    ) {
        self.commands.push(DebugDrawCommand::Line {
            from,
            to,
            color: color.into_gpu_color(),
            thickness,
            layer,
            draw_space,
        });
    }

    #[allow(unused)]
    pub fn draw_polyline<C: IntoGpuColor + Clone>(
        &mut self,
        points: &[IVec2],
        color: C,
        thickness: i32,
        layer: i32,
        draw_space: DrawSpace,
    ) {
        for pts in points.windows(2) {
            self.commands.push(DebugDrawCommand::Line {
                from: pts[0],
                to: pts[1],
                color: color.clone().into_gpu_color(),
                thickness,
                layer,
                draw_space,
            });
        }
    }

    #[allow(unused, clippy::too_many_arguments)]
    pub fn draw_rect<C: IntoGpuColor + Clone>(
        &mut self,
        topleft: IVec2,
        size: IVec2,
        angle_rad: f32,
        color: C,
        thickness: i32,
        layer: i32,
        draw_space: DrawSpace,
    ) {
        let center = topleft + ivec2(size.x / 2, -size.y / 2);
        self.draw_rect_center(center, size, angle_rad, color, thickness, layer, draw_space);
    }

    #[allow(unused, clippy::too_many_arguments)]
    pub fn draw_rect_center<C: IntoGpuColor + Clone>(
        &mut self,
        center: IVec2,
        size: IVec2,
        angle_rad: f32,
        color: C,
        thickness: i32,
        layer: i32,
        draw_space: DrawSpace,
    ) {
        let center = center.as_vec2();
        let half = size / 2;
        let rotation = glam::Mat2::from_angle(angle_rad);
        let quad_pts = [
            (center + rotation * IVec2::new(-half.x, half.y).as_vec2()).as_ivec2(),
            (center + rotation * IVec2::new(-half.x, -half.y).as_vec2()).as_ivec2(),
            (center + rotation * IVec2::new(half.x, -half.y).as_vec2()).as_ivec2(),
            (center + rotation * IVec2::new(half.x, half.y).as_vec2()).as_ivec2(),
        ];
        self.commands.push(DebugDrawCommand::Line {
            from: quad_pts[0],
            to: quad_pts[1],
            color: color.clone().into_gpu_color(),
            thickness,
            layer,
            draw_space,
        });
        self.commands.push(DebugDrawCommand::Line {
            from: quad_pts[0],
            to: quad_pts[3],
            color: color.clone().into_gpu_color(),
            thickness,
            layer,
            draw_space,
        });
        self.commands.push(DebugDrawCommand::Line {
            from: quad_pts[1],
            to: quad_pts[2],
            color: color.clone().into_gpu_color(),
            thickness,
            layer,
            draw_space,
        });
        self.commands.push(DebugDrawCommand::Line {
            from: quad_pts[2],
            to: quad_pts[3],
            color: color.into_gpu_color(),
            thickness,
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
                    thickness,
                    layer,
                    draw_space,
                } => {
                    let from = from.as_vec2();
                    let to = to.as_vec2();

                    let direction = to - from;
                    let length = direction.length();

                    if length <= f32::EPSILON {
                        continue;
                    }

                    let center = (from + to) * 0.5;
                    let angle = direction.to_angle();

                    let size = ivec2(length.round() as i32, (*thickness).max(1));

                    batch_renderer.draw_quad_with_center(
                        center.as_ivec2(),
                        size,
                        angle,
                        *color,
                        None,
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
