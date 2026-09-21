use glam::prelude::*;
use granular_core::utils::*;
use rapier2d::prelude::*;

/// The 2D simulation (running via Rapier2D).
/// Internally uses a conversion between pixels and physics units
pub(super) struct SimPhysics {
    rb_set: RigidBodySet,
    collider_set: ColliderSet,
    integration_params: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
    physics_hooks: (),
    event_handler: (),

    /// Conversion factor between pixels and physics units.
    ///
    /// Is basically "one physics meter is `scaling_factor`-many pixels"
    scaling_factor: f32,
}
impl SimPhysics {
    #[allow(unused)]
    #[inline(always)]
    fn phys_to_pix(&self, physics_units: f32) -> f32 {
        physics_units * self.scaling_factor
    }

    #[allow(unused)]
    #[inline(always)]
    fn physvec_to_pix(&self, physics_units: Vec2) -> Vec2 {
        physics_units * self.scaling_factor
    }

    #[allow(unused)]
    #[inline(always)]
    fn pix_to_phys(&self, pixels: i32) -> f32 {
        pixels as f32 / self.scaling_factor
    }

    #[allow(unused)]
    #[inline(always)]
    fn pixvec_to_phys(&self, pixels: Vec2) -> Vec2 {
        pixels / self.scaling_factor
    }

    pub(super) fn new(one_physics_meter_is_pixels: u32) -> Self {
        let scaling_factor = one_physics_meter_is_pixels as f32;

        let rb_set = RigidBodySet::new();
        let mut collider_set = ColliderSet::new();

        /* Create the ground. */
        let collider = ColliderBuilder::cuboid(100.0, 0.1)
            .translation(vec2(100.0 / scaling_factor, 0.0 / scaling_factor))
            .build();
        collider_set.insert(collider);

        let integration_params = IntegrationParameters::default();
        let physics_pipeline = PhysicsPipeline::new();
        let island_manager = IslandManager::new();
        let broad_phase = DefaultBroadPhase::new();
        let narrow_phase = NarrowPhase::new();
        let impulse_joint_set = ImpulseJointSet::new();
        let multibody_joint_set = MultibodyJointSet::new();
        let ccd_solver = CCDSolver::new();
        let physics_hooks = ();
        let event_handler = ();

        Self {
            rb_set,
            collider_set,
            integration_params,
            physics_pipeline,
            island_manager,
            broad_phase,
            narrow_phase,
            impulse_joint_set,
            multibody_joint_set,
            ccd_solver,
            physics_hooks,
            event_handler,
            scaling_factor,
        }
    }

    pub(super) fn step(&mut self) {
        let gravity = vec2(0.0, -9.81);
        self.physics_pipeline.step(
            gravity,
            &self.integration_params,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rb_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            &self.physics_hooks,
            &self.event_handler,
        );

        // Iter on each rigid-bodies that moved (dynamic and kinematic).
        // for rigid_body_handle in island_manager.active_bodies() {
        //     let rigid_body = &rigid_body_set[rigid_body_handle];
        //     println!(
        //         "Rigid body {:?} has a new position: {:?}",
        //         rigid_body_handle,
        //         rigid_body.position()
        //     );
        // }

        // Init a temporary query pipeline by borrowing from the broad-phase
        // and collides/rigid-bodies. Scene queries will take into account
        // the last objects positions know at the end of the last physics
        // simulation step.
        // let query_pipeline = broad_phase.as_query_pipeline(
        //     narrow_phase.query_dispatcher(),
        //     rigid_body_set,
        //     collider_set,
        //     filter,
        // );
    }

    fn mass_properties_from_cells(&self, cells: &[(IVec2, f32)]) -> MassProperties {
        let half_extents = self.pixvec_to_phys(Vec2::splat(0.5));

        let properties: MassProperties = cells
            .iter()
            .map(|&(cell, density)| {
                assert!(density.is_finite() && density >= 0.0);
                let center = self.pixvec_to_phys(cell.as_vec2() + Vec2::splat(0.5));
                let pixel_properties = MassProperties::from_cuboid(density, half_extents);
                pixel_properties.transform_by(&Pose2::from_translation(center))
            })
            .sum();

        assert!(properties.mass() > 0.0, "dynamic body needs positive mass",);

        properties
    }

    /// Translation/ Rotation/ Pose will get overwritten by `position`/ `angle` on the RigidBodyBuilder
    /// `collider_pixels` is a list of pixels (in local coords) and their densities
    pub(super) fn create_rigidbody(
        &mut self,
        rb_builder: RigidBodyBuilder,
        pixel_position: Vec2,
        angle: f32,
        collider_pixels: &[(IVec2, f32)],
    ) -> RigidBodyHandle {
        // Calculate bounding box of collider pixels
        let mut min = collider_pixels[0].0;
        let mut max = collider_pixels[0].0;
        for &pix in collider_pixels {
            min = min.min(pix.0);
            max = max.max(pix.0);
        }

        let bounds_min = min.as_vec2();
        let bounds_max = max.as_vec2() + Vec2::ONE;
        let bounds_center = (bounds_min + bounds_max) * 0.5;
        let bounds_size = bounds_max - bounds_min;

        // How much to grow the collider on each side
        const GROW_BY_SIM_PIXELS: f32 = 0.0;

        let required_scale = (bounds_size + Vec2::splat(2.0 * GROW_BY_SIM_PIXELS)) / bounds_size;
        let scale = Vec2::splat(required_scale.max_element());
        let offset_pixels = bounds_center * (Vec2::ONE - scale);

        let props = self.mass_properties_from_cells(collider_pixels);
        let voxels: Vec<IVec2> = collider_pixels.iter().map(|v| v.0).collect();
        let collider = ColliderBuilder::voxels(self.pixvec_to_phys(Vec2::ONE) * scale, &voxels)
            .translation(self.pixvec_to_phys(offset_pixels))
            .mass_properties(props)
            .build();

        let mut pose = Pose2::from_translation(self.pixvec_to_phys(pixel_position));
        pose.rotation = Rot2::from_angle(angle);

        let rb = rb_builder.pose(pose).build();
        let rb_handle = self.rb_set.insert(rb);

        self.collider_set
            .insert_with_parent(collider, rb_handle, &mut self.rb_set);

        rb_handle
    }

    /// Returns that Rigidbodies position in pixel units as well as its rotation
    pub(super) fn get_rigidbody_pose(&self, rb_handle: RigidBodyHandle) -> (Vec2, f32) {
        let rb = &self.rb_set[rb_handle];
        (self.physvec_to_pix(rb.translation()), rb.rotation().angle())
    }

    pub(super) fn get_rigidbody(&mut self, handle: RigidBodyHandle) -> &mut RigidBody {
        &mut self.rb_set[handle]
    }

    pub(super) fn draw_each_collider<T>(
        &self,
        target: &mut T,
        mut draw_quad_center: impl FnMut(&mut T, Vec2, Vec2, f32),
        mut draw_circle: impl FnMut(&mut T, Vec2, f32),
        mut draw_polyline: impl FnMut(&mut T, &[Vec2]),
    ) {
        for (_, collider) in self.collider_set.iter() {
            let pose = collider.position();
            let center_pixels = self.physvec_to_pix(pose.translation);
            let angle = pose.rotation.angle();

            if let Some(voxels) = collider.shape().as_voxels() {
                let size_pixels = self.physvec_to_pix(voxels.voxel_size());
                for voxel in voxels.voxels() {
                    if voxel.state.is_empty() {
                        continue;
                    }

                    let voxel_center_pixels =
                        self.physvec_to_pix(pose.transform_point(voxel.center));

                    draw_quad_center(target, voxel_center_pixels, size_pixels, angle);
                }
            } else if let Some(ball) = collider.shape().as_ball() {
                draw_circle(target, center_pixels, self.phys_to_pix(ball.radius));
            } else if let Some(cube) = collider.shape().as_cuboid() {
                draw_quad_center(
                    target,
                    center_pixels,
                    self.physvec_to_pix(cube.half_extents * 2.0),
                    angle,
                );
            } else if let Some(polyline) = collider.shape().as_polyline() {
                for segment in polyline.indices() {
                    let points = segment.map(|index| {
                        self.physvec_to_pix(
                            pose.transform_point(polyline.vertices()[index as usize]),
                        )
                    });

                    draw_polyline(target, &points);
                }
            }
        }
    }
}
