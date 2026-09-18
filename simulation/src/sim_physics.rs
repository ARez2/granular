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

    ball_body_handle: RigidBodyHandle,
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

    pub(super) fn new(one_physics_meter_is_pixels: u32) -> Self {
        let scaling_factor = one_physics_meter_is_pixels as f32;

        let mut rb_set = RigidBodySet::new();
        let mut collider_set = ColliderSet::new();

        /* Create the ground. */
        let collider = ColliderBuilder::cuboid(100.0, 0.1)
            .translation(vec2(100.0 / scaling_factor, 0.0))
            .build();
        collider_set.insert(collider);

        /* Create the bouncing ball. */
        let rigid_body = RigidBodyBuilder::dynamic()
            .translation(Vector::new(100.0 / scaling_factor, 150.0 / scaling_factor))
            .build();
        let collider = ColliderBuilder::ball(20.0 / scaling_factor)
            .restitution(1.0)
            .build();
        let ball_body_handle = rb_set.insert(rigid_body);
        collider_set.insert_with_parent(collider, ball_body_handle, &mut rb_set);

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

            ball_body_handle,
        }
    }

    pub(super) fn step(&mut self) {
        self.physics_pipeline.step(
            vec2(0.0, -9.81),
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

    pub(super) fn get_ball_pos(&self) -> IVec2 {
        self.physvec_to_pix(self.rb_set[self.ball_body_handle].translation())
            .as_ivec2()
    }
}
