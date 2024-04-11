use std::sync::{Arc, Mutex};

use ratatui::{
    prelude::*,
    widgets::{Clear, Paragraph},
};

use transmission_rpc::types::Torrent;
use tui_input::{Input, InputRequest};

use crate::{
    action::Action,
    app,
    ui::components::{table::GenericTable, Component},
};

use super::tasks::{add_magnet::AddMagnetBar, delete_torrent::DeleteBar};

pub struct TaskManager {
    ctx: app::Ctx,
    current_task: CurrentTask,
    table: Arc<Mutex<GenericTable<Torrent>>>,
}

impl TaskManager {
    pub const fn new(table: Arc<Mutex<GenericTable<Torrent>>>, ctx: app::Ctx) -> Self {
        Self {
            ctx,
            current_task: CurrentTask::None,
            table,
        }
    }

    #[must_use]
    fn handle_events_to_self(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::AddMagnet => {
                self.current_task = CurrentTask::AddMagnetBar(AddMagnetBar::new(self.ctx.clone()));
                Some(Action::SwitchToInputMode)
            }
            Action::Delete => {
                self.current_task = CurrentTask::DeleteBar(DeleteBar::new(
                    self.ctx.clone(),
                    vec![self
                        .table
                        .lock()
                        .unwrap()
                        .current_item()
                        .unwrap()
                        .id()
                        .unwrap()],
                ));
                Some(Action::SwitchToInputMode)
            }
            _ => None,
        }
    }

    fn finish_task(&mut self) -> Option<Action> {
        match self.current_task {
            CurrentTask::AddMagnetBar(_) => {
                self.current_task = CurrentTask::None;
                Some(Action::SwitchToNormalMode)
            }
            CurrentTask::DeleteBar(_) => {
                self.current_task = CurrentTask::None;
                Some(Action::SwitchToNormalMode)
            }
            CurrentTask::None => None,
        }
    }
}

enum CurrentTask {
    AddMagnetBar(AddMagnetBar),
    DeleteBar(DeleteBar),
    None,
}

impl Component for TaskManager {
    #[must_use]
    fn handle_actions(&mut self, action: Action) -> Option<Action> {
        match &mut self.current_task {
            CurrentTask::AddMagnetBar(magnet_bar) => match magnet_bar.handle_actions(action) {
                Some(Action::Quit) => self.finish_task(),

                Some(Action::Render) => Some(Action::Render),

                _ => None,
            },

            CurrentTask::DeleteBar(delete_bar) => match delete_bar.handle_actions(action) {
                Some(Action::Quit) => self.finish_task(),

                Some(Action::Render) => Some(Action::Render),

                _ => None,
            },

            CurrentTask::None => self.handle_events_to_self(&action),
        }
    }

    fn render(&mut self, f: &mut Frame, rect: Rect) {
        match &mut self.current_task {
            CurrentTask::AddMagnetBar(magnet_bar) => magnet_bar.render(f, rect),
            CurrentTask::DeleteBar(delete_bar) => delete_bar.render(f, rect),
            CurrentTask::None => (),
        }
    }
}

pub struct InputManager {
    input: Input,
    prompt: String,
}

impl InputManager {
    pub fn new(prompt: String) -> Self {
        Self {
            prompt,
            input: Input::default(),
        }
    }

    pub fn text(&self) -> String {
        self.input.to_string()
    }

    pub fn handle(&mut self, req: InputRequest) {
        self.input.handle(req);
    }
}

impl Component for InputManager {
    fn handle_actions(&mut self, _action: Action) -> Option<Action> {
        None
    }

    fn render(&mut self, f: &mut Frame, rect: Rect) {
        f.render_widget(Clear, rect);

        let paragraph_text = format!("{}{}", self.prompt, self.text());

        let input = self.input.to_string();
        let prefix_len = paragraph_text.len() - input.len();

        let paragraph = Paragraph::new(paragraph_text);
        f.render_widget(paragraph, rect);

        let cursor_offset = self.input.visual_cursor() + prefix_len;
        f.set_cursor(rect.x + u16::try_from(cursor_offset).unwrap(), rect.y);
    }
}