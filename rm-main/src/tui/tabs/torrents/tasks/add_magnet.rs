use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use rm_config::CONFIG;

use crate::{
    transmission::TorrentAction,
    tui::{
        components::{Component, ComponentAction, InputManager},
        ctx::CTX,
        tabs::torrents::SESSION_GET,
    },
};
use rm_shared::{
    action::{Action, UpdateAction},
    status_task::StatusTask,
};

pub struct AddMagnet {
    input_magnet_mgr: InputManager,
    input_category_mgr: InputManager,
    input_location_mgr: InputManager,
    stage: Stage,
}

enum Stage {
    Magnet,
    Category,
    Location,
}

const MAGNET_PROMPT: &str = "Add magnet URI: ";
const CATEGORY_PROMPT: &str = "Category (empty for default): ";
const LOCATION_PROMPT: &str = "Directory: ";

impl AddMagnet {
    pub fn new() -> Self {
        Self {
            input_magnet_mgr: InputManager::new(MAGNET_PROMPT.to_string()),
            input_category_mgr: InputManager::new(CATEGORY_PROMPT.to_string())
                .autocompletions(CONFIG.categories.map.keys().cloned().collect()),
            input_location_mgr: InputManager::new_with_value(
                LOCATION_PROMPT.to_string(),
                SESSION_GET.get().unwrap().download_dir.clone(),
            ),
            stage: Stage::Magnet,
        }
    }

    pub fn magnet(mut self, magnet: impl Into<String>) -> Self {
        self.input_magnet_mgr.set_text(magnet);
        if CONFIG.categories.is_empty() {
            self.stage = Stage::Location
        } else {
            self.stage = Stage::Category;
        }

        self
    }

    fn handle_input(&mut self, input: KeyEvent) -> ComponentAction {
        match self.stage {
            Stage::Magnet => self.handle_magnet_input(input),
            Stage::Category => self.handle_category_input(input),
            Stage::Location => self.handle_location_input(input),
        }
    }

    fn handle_magnet_input(&mut self, input: KeyEvent) -> ComponentAction {
        if input.code == KeyCode::Enter {
            if CONFIG.categories.is_empty() {
                self.stage = Stage::Location;
            } else {
                self.stage = Stage::Category;
            }
            CTX.send_action(Action::Render);
            return ComponentAction::Nothing;
        }

        if input.code == KeyCode::Esc {
            return ComponentAction::Quit;
        }

        if self.input_magnet_mgr.handle_key(input).is_some() {
            CTX.send_action(Action::Render);
        }

        ComponentAction::Nothing
    }

    fn handle_category_input(&mut self, input: KeyEvent) -> ComponentAction {
        if input.code == KeyCode::Enter {
            if self.input_category_mgr.text().is_empty() {
                self.stage = Stage::Location;
                CTX.send_action(Action::Render);
                return ComponentAction::Nothing;
            } else if let Some(category) =
                CONFIG.categories.map.get(&self.input_category_mgr.text())
            {
                self.input_location_mgr = InputManager::new_with_value(
                    LOCATION_PROMPT.to_string(),
                    category.default_dir.to_string(),
                );
                self.stage = Stage::Location;
                CTX.send_action(Action::Render);
                return ComponentAction::Nothing;
            } else {
                self.input_category_mgr.set_prompt(format!(
                    "Category ({} not found): ",
                    self.input_category_mgr.text()
                ));
                CTX.send_action(Action::Render);
                return ComponentAction::Nothing;
            };
        }

        if input.code == KeyCode::Esc {
            return ComponentAction::Quit;
        }

        if self.input_category_mgr.handle_key(input).is_some() {
            CTX.send_action(Action::Render);
        }

        ComponentAction::Nothing
    }

    fn handle_location_input(&mut self, input: KeyEvent) -> ComponentAction {
        if input.code == KeyCode::Enter {
            let category = if self.input_category_mgr.text().is_empty() {
                None
            } else {
                Some(self.input_category_mgr.text())
            };

            let torrent_action = TorrentAction::Add(
                self.input_magnet_mgr.text(),
                Some(self.input_location_mgr.text()),
                category,
            );
            CTX.send_torrent_action(torrent_action);

            let task = StatusTask::new_add(self.input_magnet_mgr.text());
            CTX.send_update_action(UpdateAction::StatusTaskSet(task));

            ComponentAction::Quit
        } else if input.code == KeyCode::Esc {
            ComponentAction::Quit
        } else if self.input_location_mgr.handle_key(input).is_some() {
            CTX.send_action(Action::Render);
            ComponentAction::Nothing
        } else {
            ComponentAction::Nothing
        }
    }
}

impl Component for AddMagnet {
    #[must_use]
    fn handle_actions(&mut self, action: Action) -> ComponentAction {
        match action {
            Action::Input(input) => self.handle_input(input),
            _ => ComponentAction::Nothing,
        }
    }

    fn render(&mut self, f: &mut Frame, rect: Rect) {
        match self.stage {
            Stage::Magnet => self.input_magnet_mgr.render(f, rect),
            Stage::Category => self.input_category_mgr.render(f, rect),
            Stage::Location => self.input_location_mgr.render(f, rect),
        }
    }
}
