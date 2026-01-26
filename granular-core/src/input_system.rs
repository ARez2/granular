#![allow(unused)]

use glam::IVec2;
use rustc_hash::FxHashMap as HashMap;
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
    just_pressed: bool,
}
impl InputAction {
    /// Creates a new input action with just a name
    pub(crate) fn empty(name: &str) -> Self {
        Self {
            name: String::from(name),
            triggers: vec![],
            pressed: false,
            just_pressed: false,
        }
    }

    /// Creates a new input action from a trigger (name, keycode and modifiers pressed)
    pub(crate) fn new(name: &str, trigger: InputActionTrigger) -> Self {
        Self {
            name: String::from(name),
            triggers: vec![trigger],
            pressed: false,
            just_pressed: false,
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

pub struct InputSystem {
    ctx: GeeseContextHandle<Self>,
    actions: HashMap<String, InputAction>,
    current_modifiers: ModifiersState,
    mouse_position: IVec2,
    last_mouse_position: IVec2,
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
                warn!("is_action_pressed: Action '{}' does not exist. Create it by calling add_action.", name);
                false
            }
        }
    }

    /// Returns true when at least one of the triggers of an InputAction
    /// have been pressed down **this frame**
    pub fn is_action_just_pressed(&self, name: &str) -> bool {
        match self.actions.get(name) {
            Some(action) => action.just_pressed,
            None => {
                warn!("is_action_just_pressed: Action '{}' does not exist. Create it by calling add_action.", name);
                false
            }
        }
    }

    pub fn get_mouse_position(&self) -> IVec2 {
        self.mouse_position
    }

    /// Returns the change of the mouse position between this and the last frame
    pub fn get_mouse_delta(&self) -> IVec2 {
        (self.mouse_position - self.last_mouse_position) * IVec2::new(1, -1)
    }

    pub fn get_input_vector(
        &self,
        action_left: &str,
        action_right: &str,
        action_up: &str,
        action_down: &str,
    ) -> IVec2 {
        let actions = [
            (action_left, self.actions.get(action_left)),
            (action_right, self.actions.get(action_right)),
            (action_up, self.actions.get(action_up)),
            (action_down, self.actions.get(action_down)),
        ];
        for (name, action) in actions {
            if action.is_none() {
                warn!(
                    "get_input_vector: Action '{}' does not exist, create it using add_action.",
                    name
                );
                return IVec2::ZERO;
            };
        }
        IVec2::new(
            actions[1].1.unwrap().pressed as i32 - actions[0].1.unwrap().pressed as i32,
            actions[2].1.unwrap().pressed as i32 - actions[3].1.unwrap().pressed as i32,
        )
    }

    /// Updates keyboard input for all InputAction's
    pub(crate) fn handle_keyevent(&mut self, event: &KeyEvent) {
        self.actions.iter_mut().for_each(|(key, action)| {
            action.triggers.iter().for_each(|trigger| {
                if let InputActionTriggerReason::Key(trigger_key) = trigger.reason {
                    if event.physical_key == trigger_key
                        && self.current_modifiers == trigger.modifiers
                    {
                        action.just_pressed =
                            event.state == ElementState::Pressed && event.repeat == false;
                        action.pressed = event.state == ElementState::Pressed;
                    };
                };
            });
        });
    }

    /// Updates mouse input for all InputAction's
    pub(crate) fn handle_mouse_input(&mut self, button: MouseButton, state: ElementState) {
        self.actions.values_mut().for_each(|action| {
            action.triggers.iter().for_each(|trigger| {
                if let InputActionTriggerReason::Mouse(trigger_button) = trigger.reason {
                    if button == trigger_button && self.current_modifiers == trigger.modifiers {
                        action.just_pressed = state == ElementState::Pressed;
                        action.pressed = state == ElementState::Pressed;
                    };
                };
            });
        });
    }

    /// Sets the current mouse position and updates the last mouse position
    pub(crate) fn handle_cursor_movement(&mut self, new_position: PhysicalPosition<f64>) {
        let tmp = self.mouse_position;
        // new_position always ends in .0 so we can safely cast here without loosing precision
        self.mouse_position = IVec2::new(new_position.x as i32, new_position.y as i32);
        self.last_mouse_position = tmp;
    }

    pub(crate) fn update_modifiers(&mut self, modifiers: &Modifiers) {
        self.current_modifiers = modifiers.state();
    }

    /// Sets the `just_pressed` property of all InputAction's to `false`
    pub(crate) fn reset_just_pressed(&mut self) {
        self.actions.values_mut().for_each(|action| {
            action.just_pressed = false;
        });
    }
}
impl GeeseSystem for InputSystem {
    fn new(ctx: geese::GeeseContextHandle<Self>) -> Self {
        Self {
            ctx,
            actions: HashMap::default(),
            mouse_position: IVec2::ZERO,
            last_mouse_position: IVec2::ZERO,
            current_modifiers: ModifiersState::empty(),
        }
    }
}
