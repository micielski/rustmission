mod providers;

use crate::tui::components::{Component, ComponentAction};
use crate::tui::ctx::CTX;
use providers::ProvidersPopup;
use ratatui::prelude::*;
use ratatui::Frame;
use rm_shared::action::Action;

use super::ConfiguredProvider;

pub struct PopupManager {
    pub current_popup: Option<CurrentPopup>,
}

pub enum CurrentPopup {
    Providers(ProvidersPopup),
}

impl PopupManager {
    pub const fn new() -> Self {
        Self {
            current_popup: None,
        }
    }

    pub const fn is_showing_popup(&self) -> bool {
        self.current_popup.is_some()
    }

    fn show_popup(&mut self, popup: CurrentPopup) {
        self.current_popup = Some(popup);
    }

    pub fn show_providers_info_popup(&mut self, providers: Vec<ConfiguredProvider>) {
        self.show_popup(CurrentPopup::Providers(ProvidersPopup::new(providers)));
    }

    pub fn close_popup(&mut self) {
        self.current_popup = None;
    }
}

impl Component for PopupManager {
    fn handle_actions(&mut self, action: Action) -> ComponentAction {
        if let Some(current_popup) = &mut self.current_popup {
            match current_popup {
                CurrentPopup::Providers(popup) => {
                    if popup.handle_actions(action).is_quit() {
                        self.close_popup();
                        CTX.send_action(Action::Render);
                    }
                }
            }
        }

        ComponentAction::Nothing
    }

    fn render(&mut self, f: &mut Frame, rect: Rect) {
        if let Some(popup) = &mut self.current_popup {
            match popup {
                CurrentPopup::Providers(popup) => popup.render(f, rect),
            }
        }
    }
}
