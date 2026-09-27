#![allow(unused)]

use crate::graphics::SurfacePos;
use glam::Vec2;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, KeyEvent, Modifiers, MouseButton},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
};

use crate::utils::*;

pub mod events {
    use super::InputAction;

    pub struct Input(pub InputAction);
}

/// Helper enum to keep track of multiple ways an action could be triggered
pub enum InputActionTriggerReason {
    Key(KeyCode),
    Mouse(MouseButton),
}

/// Holds information about what things need to happen in order for the action to trigger
pub struct InputActionTrigger {
    reason: InputActionTriggerReason,
    modifiers: ModifiersState,
}
impl InputActionTrigger {
    /// The longest form of creating an InputActionTrigger
    pub fn new(reason: InputActionTriggerReason, modifiers: ModifiersState) -> Self {
        Self { reason, modifiers }
    }

    /// Shorthand for creating a new key InputActionTrigger
    pub fn new_key(key: KeyCode, modifiers: ModifiersState) -> Self {
        Self::new(InputActionTriggerReason::Key(key), modifiers)
    }

    /// Shorthand for creating a new InputActionTrigger, for including a modifier, see new_mouse_mod
    pub fn new_mouse(mouse_button: MouseButton) -> Self {
        Self::new_mouse_mod(mouse_button, ModifiersState::empty())
    }

    /// Creates a new mouse button InputActionTrigger together with a modifier (for example Ctrl + LMB)
    pub fn new_mouse_mod(mouse_button: MouseButton, modifiers: ModifiersState) -> Self {
        Self::new(InputActionTriggerReason::Mouse(mouse_button), modifiers)
    }
}

/// An named input which knows if it has been pressed and can have multiple triggers
pub struct InputAction {
    name: String,
    triggers: Vec<InputActionTrigger>,

    pressed: bool,
}
impl InputAction {
    /// Creates a new input action with just a name
    pub(crate) fn empty(name: &str) -> Self {
        Self {
            name: String::from(name),
            triggers: vec![],
            pressed: false,
        }
    }

    /// Creates a new input action from a trigger (name, keycode and modifiers pressed)
    pub(crate) fn new(name: &str, trigger: InputActionTrigger) -> Self {
        Self {
            name: String::from(name),
            triggers: vec![trigger],
            pressed: false,
        }
    }

    /// Returns the name of the InputAction
    pub fn name(&self) -> &String {
        &self.name
    }

    /// Adds a new trigger to the list of triggers
    pub fn add_trigger(&mut self, trigger: InputActionTrigger) {
        self.triggers.push(trigger);
    }

    /// Removes the trigger at that index
    pub fn remove_trigger(&mut self, index: usize) {
        self.triggers.remove(index);
    }

    /// Returns how many triggers there are for this InputAction
    /// useful for using with remove_trigger
    pub fn num_triggers(&self) -> usize {
        self.triggers.len()
    }
}

/// This stores if an action just_pressed and/or released between two snapshots.
/// Both can be true (like a quick tap between two long ticks)
#[derive(Clone, Copy, Default)]
struct InputActionChanges {
    just_pressed: bool,
    just_released: bool,
}

#[derive(Default)]
struct InputSnapshot {
    pending: HashMap<String, InputActionChanges>,
    current: HashMap<String, InputActionChanges>,
    last_mouse_position: Vec2,
    mouse_delta: Vec2,
}

impl InputSnapshot {
    fn record(&mut self, name: &str, pressed: bool) {
        let changes = self.pending.entry(name.to_owned()).or_default();
        if pressed {
            changes.just_pressed = true;
        } else {
            changes.just_released = true;
        }
    }

    fn begin(&mut self, mouse_position: Vec2) {
        std::mem::swap(&mut self.current, &mut self.pending);
        self.pending.clear();
        self.mouse_delta = mouse_position - self.last_mouse_position;
        self.last_mouse_position = mouse_position;
    }
}

/// Each update clock receives its own copy of action changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum InputClock {
    Frame,
    Physics,
    Fixed(u64),
}

pub struct InputSystem {
    ctx: GeeseContextHandle<Self>,
    /// Stores the mapping of action name to actual action data
    actions: HashMap<String, InputAction>,
    /// Stores which modifiers are currently active
    current_modifiers: ModifiersState,
    /// This is just the current mouse position. Nothing more.
    mouse_position: Vec2,
    held_keys: HashSet<KeyCode>,
    held_mouse_buttons: HashSet<MouseButton>,
    /// For each kind of tick, we store a InputClock. And for each one, we need to store a separate snapshot
    /// of inputs which happened between the last and current occurence of that tick.
    snapshots: HashMap<InputClock, InputSnapshot>,
    /// We store which clock is currently active, so when someone asks "was this key pressed?",
    /// we can give a different answer based on what tick we are on currently
    active_clock: Option<InputClock>,
}
impl InputSystem {
    /// Registers a new InputAction
    pub fn add_action(&mut self, name: &str, trigger: InputActionTrigger) {
        if !self.actions.contains_key(name) {
            self.actions
                .insert(String::from(name), InputAction::new(name, trigger));
        } else {
            warn!("add_action: An action with that name already exists!");
        };
    }

    /// Returns true when at least one of the triggers of an InputAction
    /// are pressed down
    pub fn is_action_pressed(&self, name: &str) -> bool {
        match self.actions.get(name) {
            Some(action) => action.pressed,
            None => {
                warn!(
                    "is_action_pressed: Action '{}' does not exist. Create it by calling add_action.",
                    name
                );
                false
            }
        }
    }

    /// Returns `true` if the action was pressed between the last tick and this one
    ///
    /// Note this does not exclude `is_action_just_released() == true`! Depending on the interval
    /// of the tick, an action can be pressed and released between two ticks!
    pub fn is_action_just_pressed(&self, name: &str) -> bool {
        self.action_changes(name).just_pressed
    }

    /// Returns `true` if the action was released between the last tick and this one.
    ///
    /// Note this does not exclude `is_action_just_pressed() == true`! Depending on the interval
    /// of the tick, an action can be pressed and released between two ticks!
    pub fn is_action_just_released(&self, name: &str) -> bool {
        self.action_changes(name).just_released
    }

    /// Fetches what changes occured for this action between this and last tick
    fn action_changes(&self, name: &str) -> InputActionChanges {
        if !self.actions.contains_key(name) {
            warn!("Input action '{}' does not exist", name);
            return InputActionChanges::default();
        }
        self.snapshots
            .get(&self.active_clock.unwrap_or(InputClock::Frame))
            // Use an then, since we dont want Option<Option<...>>
            // this would be the same as map(...).flatten()
            .and_then(|snapshot| snapshot.current.get(name))
            .copied()
            .unwrap_or_default()
    }

    /// Physical surface pixels, top-left origin, +Y down.
    pub fn mouse_surface_position(&self) -> SurfacePos {
        SurfacePos(self.mouse_position)
    }

    /// Movement since this clock last ran; fixed ticks and frames are independent.
    pub fn mouse_surface_delta(&self) -> Vec2 {
        self.snapshots
            .get(&self.active_clock.unwrap_or(InputClock::Frame))
            .map_or(Vec2::ZERO, |snapshot| snapshot.mouse_delta)
    }

    /// Starts a normal frame clock
    pub(crate) fn begin_update(&mut self) {
        self.begin_clock(InputClock::Frame);
    }

    /// Starts a physics clock
    pub(crate) fn begin_physics_tick(&mut self) {
        self.begin_clock(InputClock::Physics);
    }

    /// Starts the clock for a specific fixed-millisecond tick
    pub(crate) fn begin_fixed_tick(&mut self, milliseconds: u64) {
        self.begin_clock(InputClock::Fixed(milliseconds));
    }

    /// Sets the clock as active and refreshes/ sets up the snapshot
    fn begin_clock(&mut self, clock: InputClock) {
        self.snapshots
            .get_mut(&clock)
            .expect("Input clock must be registered before receiving input")
            .begin(self.mouse_position);
        self.active_clock = Some(clock);
    }

    /// Called only after all listeners and their queued events have completed.
    pub(crate) fn end_update(&mut self) {
        self.active_clock = None;
    }

    /// Normalized keyboard direction in world axes: up is +Y.
    pub fn world_input_direction(
        &self,
        action_left: &str,
        action_right: &str,
        action_up: &str,
        action_down: &str,
    ) -> Vec2 {
        let actions = [
            (action_left, self.actions.get(action_left)),
            (action_right, self.actions.get(action_right)),
            (action_up, self.actions.get(action_up)),
            (action_down, self.actions.get(action_down)),
        ];
        for (name, action) in actions {
            if action.is_none() {
                warn!(
                    "world_input_direction: Action '{}' does not exist, create it using add_action.",
                    name
                );
                return Vec2::ZERO;
            };
        }
        Vec2::new(
            actions[1].1.unwrap().pressed as u8 as f32 - actions[0].1.unwrap().pressed as u8 as f32,
            actions[2].1.unwrap().pressed as u8 as f32 - actions[3].1.unwrap().pressed as u8 as f32,
        )
        .normalize_or_zero()
    }

    /// Track physical state, then aggregate all bindings for each action.
    pub(crate) fn handle_keyevent(&mut self, event: &KeyEvent) {
        if let PhysicalKey::Code(key) = event.physical_key {
            match event.state {
                ElementState::Pressed => {
                    self.held_keys.insert(key);
                }
                ElementState::Released => {
                    self.held_keys.remove(&key);
                }
            }
            // Repeated key-down events do not create a new action transition.
            self.refresh_actions();
        }
    }

    pub(crate) fn handle_mouse_input(&mut self, button: MouseButton, state: ElementState) {
        match state {
            ElementState::Pressed => {
                self.held_mouse_buttons.insert(button);
            }
            ElementState::Released => {
                self.held_mouse_buttons.remove(&button);
            }
        }
        self.refresh_actions();
    }

    /// Uses the collected input state to update the state of the InputActions
    fn refresh_actions(&mut self) {
        for action in self.actions.values_mut() {
            let pressed = action.triggers.iter().any(|trigger| {
                self.current_modifiers == trigger.modifiers
                    && match trigger.reason {
                        InputActionTriggerReason::Key(key) => self.held_keys.contains(&key),
                        InputActionTriggerReason::Mouse(button) => {
                            self.held_mouse_buttons.contains(&button)
                        }
                    }
            });
            if pressed != action.pressed {
                action.pressed = pressed;
                for snapshot in self.snapshots.values_mut() {
                    snapshot.record(&action.name, pressed);
                }
            }
        }
    }

    pub(crate) fn handle_cursor_movement(&mut self, p: PhysicalPosition<f64>) {
        self.mouse_position = Vec2::new(p.x as f32, p.y as f32);
    }

    pub(crate) fn update_modifiers(&mut self, modifiers: &Modifiers) {
        self.current_modifiers = modifiers.state();
        self.refresh_actions();
    }

    pub(crate) fn release_all(&mut self) {
        self.held_keys.clear();
        self.held_mouse_buttons.clear();
        self.current_modifiers = ModifiersState::empty();
        self.refresh_actions();
    }
}
impl GeeseSystem for InputSystem {
    fn new(ctx: geese::GeeseContextHandle<Self>) -> Self {
        let mut snapshots = HashMap::default();
        snapshots.insert(InputClock::Frame, InputSnapshot::default());
        snapshots.insert(InputClock::Physics, InputSnapshot::default());
        for milliseconds in crate::events::timing::FIXED_TICKS {
            snapshots.insert(InputClock::Fixed(milliseconds), InputSnapshot::default());
        }
        Self {
            ctx,
            actions: HashMap::default(),
            mouse_position: Vec2::ZERO,
            held_keys: HashSet::default(),
            held_mouse_buttons: HashSet::default(),
            snapshots,
            active_clock: None,
            current_modifiers: ModifiersState::empty(),
        }
    }
}
