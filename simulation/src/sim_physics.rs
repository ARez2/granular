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
            .translation(vec2(100.0 / scaling_factor, 0.0))
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

    /// Translation/ Rotation/ Pose will get overwritten by `position`/ `angle` on the RigidBodyBuilder
    pub(super) fn create_rigidbody(
        &mut self,
        rb_builder: RigidBodyBuilder,
        pixel_position: Vec2,
        angle: f32,
        collider_pixels: &[IVec2],
    ) -> RigidBodyHandle {
        let collider =
            ColliderBuilder::voxels(self.pixvec_to_phys(Vec2::ONE), collider_pixels).build();

        let mut pose = Pose2::from_translation(self.pixvec_to_phys(pixel_position));
        pose.rotation = Rot2::from_angle(angle);

        let rb_handle = self.rb_set.insert(rb_builder.pose(pose).build());

        self.collider_set
            .insert_with_parent(collider, rb_handle, &mut self.rb_set);

        rb_handle
    }

    /// Returns that Rigidbodies position in pixel units as well as its rotation
    pub(super) fn get_rigidbody_pose(&self, rb_handle: RigidBodyHandle) -> (Vec2, f32) {
        let rb = &self.rb_set[rb_handle];
        (self.physvec_to_pix(rb.translation()), rb.rotation().angle())
    }

    pub(super) fn for_each_collider_voxel(
        &self,
        rb_handle: RigidBodyHandle,
        mut visit: impl FnMut(Vec2, Vec2, f32),
    ) {
        let rb = &self.rb_set[rb_handle];

        for &handle in rb.colliders() {
            let collider = &self.collider_set[handle];
            let Some(voxels) = collider.shape().as_voxels() else {
                continue;
            };

            let pose = *rb.position()
                * *collider
                    .position_wrt_parent()
                    .expect("attached collider has a local pose");

            let size_pixels = self.physvec_to_pix(voxels.voxel_size());

            for voxel in voxels.voxels() {
                if voxel.state.is_empty() {
                    continue;
                }

                let center_pixels = self.physvec_to_pix(pose.transform_point(voxel.center));
                visit(center_pixels, size_pixels, pose.rotation.angle());
            }
        }
    }
}
