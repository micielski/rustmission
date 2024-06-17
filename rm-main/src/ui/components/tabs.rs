use crate::{action::Action, app};

use super::Component;
use ratatui::{layout::Flex, prelude::*, widgets::Tabs};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CurrentTab {
    Torrents = 0,
    Search,
}

pub struct TabComponent {
    tabs_list: [&'static str; 2],
    pub current_tab: CurrentTab,
    ctx: app::Ctx,
}

impl TabComponent {
    pub const fn new(ctx: app::Ctx) -> Self {
        Self {
            ctx,
            tabs_list: ["Torrents", "Search"],
            current_tab: CurrentTab::Torrents,
        }
    }
}

impl Component for TabComponent {
    fn render(&mut self, f: &mut Frame, rect: Rect) {
        let center_rect = Layout::horizontal([Constraint::Length(18)])
            .flex(Flex::Center)
            .split(rect)[0];

        let tabs_highlight_style =
            Style::default().fg(self.ctx.config.general.accent_color.as_ratatui());
        let tabs = Tabs::new(self.tabs_list)
            .style(Style::default().white())
            .highlight_style(tabs_highlight_style)
            .select(self.current_tab as usize)
            .divider(symbols::DOT);

        f.render_widget(tabs, center_rect);
    }

    fn handle_actions(&mut self, action: Action) -> Option<Action> {
        if let Action::ChangeTab(tab) = action {
            match tab {
                1 => self.current_tab = CurrentTab::Torrents,
                2 => self.current_tab = CurrentTab::Search,
                _ => (),
            }
        }
        None
    }
}
