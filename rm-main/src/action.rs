use crossterm::event::{KeyCode, KeyEvent};
use transmission_rpc::types::{SessionStats, Torrent};

use crate::{tui::Event, ui::ErrorPopup};

#[derive(Debug, Clone)]
pub enum Action {
    Tick,
    Quit,
    Render,
    Up,
    Down,
    Confirm,
    SwitchToInputMode,
    SwitchToNormalMode,
    AddMagnet,
    Input(KeyEvent),
    Error(Box<ErrorPopup>),
    TorrentListUpdate(Box<Vec<Torrent>>),
    StatsUpdate(Box<SessionStats>),
    TorrentAdd(Box<String>),
}

impl Action {
    pub const fn is_render(&self) -> bool {
        matches!(self, Self::Render)
    }
}

#[derive(Clone, Copy)]
pub enum Mode {
    Input,
    Normal,
}

pub const fn event_to_action(mode: Mode, event: Event) -> Option<Action> {
    match event {
        Event::Quit => Some(Action::Quit),
        Event::Error => todo!(),
        Event::Tick => Some(Action::Tick),
        Event::Render => Some(Action::Render),
        Event::Key(key) if matches!(mode, Mode::Input) => Some(Action::Input(key)),
        Event::Key(_) => keycode_to_action(event),
    }
}

const fn keycode_to_action(event: Event) -> Option<Action> {
    if let Event::Key(key) = event {
        return match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Action::Down),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::Up),
            KeyCode::Char('m') => Some(Action::AddMagnet),
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Enter => Some(Action::Confirm),
            _ => None,
        };
    }
    None
}
