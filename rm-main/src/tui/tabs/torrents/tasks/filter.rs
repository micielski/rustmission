use crossterm::event::KeyCode;
use ratatui::prelude::*;

use rm_shared::action::{Action, UpdateAction};

use crate::tui::{
    components::{Component, ComponentAction, InputManager},
    ctx::CTX,
};

pub struct Filter {
    input: InputManager,
}

impl Filter {
    pub fn new(current_pattern: &Option<String>) -> Self {
        let pattern = current_pattern.as_ref().cloned().unwrap_or_default();
        let input = InputManager::new_with_value("Search: ".to_string(), pattern);
        Self { input }
    }
}

impl Component for Filter {
    fn handle_actions(&mut self, action: Action) -> ComponentAction {
        match action {
            Action::Input(input) => {
                if matches!(input.code, KeyCode::Enter | KeyCode::Esc) {
                    if self.input.text().is_empty() {
                        CTX.send_update_action(UpdateAction::SearchFilterClear);
                    }
                    ComponentAction::Quit
                } else if self.input.handle_key(input).is_some() {
                    CTX.send_update_action(UpdateAction::SearchFilterApply(self.input.text()));
                    ComponentAction::Nothing
                } else {
                    ComponentAction::Nothing
                }
            }
            _ => ComponentAction::Nothing,
        }
    }

    fn render(&mut self, f: &mut Frame, rect: Rect) {
        self.input.render(f, rect);
    }
}
