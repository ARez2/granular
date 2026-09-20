use super::*;

fn close(a: Vec2, b: Vec2) {
    assert!((a - b).abs().max_element() < 1e-3, "{a:?} != {b:?}");
}
fn setup(motion: CameraMotion) -> (Camera2D, Presentation) {
    let camera = Camera2D {
        position: Vec2::new(160.0, 90.0),
        motion,
        ..Default::default()
    };
    let presentation = Presentation::new(UVec2::new(320, 180), UVec2::new(1280, 720), 2.0);
    (camera, presentation)
}
#[test]
fn world_y_up_surface_y_down_and_dpi_are_explicit() {
    let (camera, presentation) = setup(CameraMotion::Free);
    let view = RenderView::new(camera, presentation);
    close(
        view.world_to_surface(WorldPos(camera.position)).0,
        Vec2::new(640.0, 360.0),
    );
    close(
        view.world_to_surface(WorldPos(camera.position + Vec2::Y)).0,
        Vec2::new(640.0, 356.0),
    );
    close(
        view.world_to_surface(WorldPos(Vec2::new(0.0, 180.0))).0,
        Vec2::ZERO,
    );
    close(
        view.ui_points_to_surface(UiPos(Vec2::new(10.0, 20.0))).0,
        Vec2::new(20.0, 40.0),
    );
    close(
        view.surface_to_ui_points(SurfacePos(Vec2::new(20.0, 40.0)))
            .0,
        Vec2::new(10.0, 20.0),
    );
}
#[test]
fn roundtrips_with_rotation_zoom_and_letterbox() {
    for motion in [
        CameraMotion::Free,
        CameraMotion::PixelSnapped,
        CameraMotion::SmoothPixel,
    ] {
        let (mut camera, mut presentation) = setup(motion);
        camera.position = Vec2::new(-20.35, 7.65);
        camera.rotation = 0.37;
        camera.zoom = 1.7;
        presentation.surface_size = UVec2::new(1100, 800);
        presentation.scaling_mode = ScalingMode::Integer;
        let view = RenderView::new(camera, presentation);
        for delta in [Vec2::ZERO, Vec2::new(10.2, -5.3), Vec2::new(-13.0, 9.0)] {
            let p = WorldPos(camera.position + delta);
            let surface = view.world_to_surface(p);
            close(view.surface_to_world(surface).unwrap().0, p.0);
            close(
                view.game_pixels_to_world(view.world_to_game_pixels(p)).0,
                p.0,
            );
            close(
                view.game_pixels_to_surface(view.surface_to_game_pixels(surface).unwrap())
                    .0,
                surface.0,
            );
        }
        assert!(view.surface_to_world(SurfacePos(Vec2::ZERO)).is_none());
        let base = view.world_to_surface(WorldPos(camera.position));
        let delta = Vec2::new(7.0, -11.0);
        close(
            view.surface_delta_to_world(delta),
            view.surface_to_world(SurfacePos(base.0 + delta)).unwrap().0
                - view.surface_to_world(base).unwrap().0,
        );
    }
}
#[test]
fn snapping_aligns_odd_and_even_resolutions_without_changing_camera() {
    for game in [UVec2::new(320, 180), UVec2::new(321, 181)] {
        let (mut camera, mut presentation) = setup(CameraMotion::PixelSnapped);
        camera.position = Vec2::new(10.3, 20.2);
        presentation.game_size = game;
        let view = RenderView::new(camera, presentation);
        let origin = view.world_to_game_pixels(WorldPos(Vec2::ZERO)).0;
        close(origin, origin.round());
        assert_eq!(camera.position, Vec2::new(10.3, 20.2));
    }
}
#[test]
fn smooth_motion_compensates_one_surface_pixel() {
    let (mut camera, presentation) = setup(CameraMotion::SmoothPixel);
    camera.position.x += 0.25;
    let view = RenderView::new(camera, presentation);
    close(view.presentation_offset, Vec2::new(-1.0, 0.0));
    close(
        view.world_to_surface(WorldPos(Vec2::new(160.0, 90.0))).0,
        Vec2::new(639.0, 360.0),
    );
}
#[test]
fn overscan_covers_residual_and_sampling_matches_world_overlay() {
    for factor in [0.5, 1.0, 2.0, 2.5, 4.0] {
        for i in 0..101 {
            let (mut camera, mut presentation) = setup(CameraMotion::SmoothPixel);
            camera.position += Vec2::new(i as f32 / 100.0, -i as f32 / 100.0);
            presentation.surface_size = (presentation.game_size.as_vec2() * factor).as_uvec2();
            let view = RenderView::new(camera, presentation);
            let uv = view.game_texture_uv_rect();
            assert!(uv.position.min_element() >= -1e-6);
            assert!((uv.position + uv.size).max_element() <= 1.0 + 1e-6);
            let p = WorldPos(Vec2::new(150.0, 95.0));
            let surface = view.world_to_surface(p).0;
            let local = (surface - view.game_viewport_surface_px.origin.as_vec2())
                / view.game_viewport_surface_px.size.as_vec2();
            let sampled_game = (uv.position + local * uv.size)
                * game_target_size(presentation.game_size).as_vec2()
                - Vec2::splat(GAME_PADDING as f32);
            close(sampled_game, view.world_to_game_pixels(p).0);
        }
    }
}
#[test]
fn small_surface_integer_fallback_and_half_open_bounds() {
    let (_, mut p) = setup(CameraMotion::Free);
    p.surface_size = UVec2::new(160, 100);
    p.scaling_mode = ScalingMode::Integer;
    let rect = p.game_viewport_surface_px();
    assert_eq!(rect.size, UVec2::new(160, 90));
    assert_eq!(rect.origin, UVec2::new(0, 5));
    assert!(rect.contains(Vec2::new(0.0, 5.0)));
    assert!(!rect.contains(Vec2::new(160.0, 5.0)));
    assert!(!rect.contains(Vec2::new(0.0, 95.0)));
}
#[test]
fn gpu_clip_and_cpu_surface_coordinates_agree() {
    let (mut camera, mut presentation) = setup(CameraMotion::SmoothPixel);
    camera.position += Vec2::new(0.25, 0.35);
    camera.rotation = -0.4;
    presentation.surface_size = UVec2::new(1400, 900);
    let view = RenderView::new(camera, presentation);
    let p = WorldPos(camera.position + Vec2::new(3.0, 5.0));
    let clip = view.world_to_surface_clip() * Vec4::new(p.0.x, p.0.y, 0.0, 1.0);
    let surface =
        Vec2::new((clip.x + 1.0) * 0.5, (1.0 - clip.y) * 0.5) * presentation.surface_size.as_vec2();
    close(surface, view.world_to_surface(p).0);
    let clip = view.world_to_game_clip() * Vec4::new(p.0.x, p.0.y, 0.0, 1.0);
    let target_pixel = Vec2::new((clip.x + 1.0) * 0.5, (1.0 - clip.y) * 0.5)
        * game_target_size(presentation.game_size).as_vec2();
    close(
        target_pixel,
        view.world_to_game_pixels(p).0 + Vec2::splat(GAME_PADDING as f32),
    );
}
#[test]
fn resize_changes_projection_and_ui_uses_only_dpi_and_ui_scale() {
    let (camera, mut p) = setup(CameraMotion::PixelSnapped);
    p.scaling_mode = ScalingMode::Integer;
    let old = RenderView::new(camera, p);
    p.game_size = UVec2::new(640, 360);
    let resized = RenderView::new(camera, p);
    assert_eq!(game_target_size(p.game_size), UVec2::new(642, 362));
    close(
        old.world_to_surface(WorldPos(camera.position + Vec2::X)).0,
        Vec2::new(644.0, 360.0),
    );
    close(
        resized
            .world_to_surface(WorldPos(camera.position + Vec2::X))
            .0,
        Vec2::new(642.0, 360.0),
    );
    close(
        old.ui_points_to_surface(UiPos(Vec2::ONE)).0,
        resized.ui_points_to_surface(UiPos(Vec2::ONE)).0,
    );
    p.dpi_scale = 1.5;
    p.ui_scale = 2.0;
    let ui_scaled = RenderView::new(camera, p);
    close(
        ui_scaled.ui_points_to_surface(UiPos(Vec2::ONE)).0,
        Vec2::splat(3.0),
    );
}
