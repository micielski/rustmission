mod bottom_stats;
pub mod popups;
pub mod rustmission_torrent;
pub mod table_manager;
pub mod task_manager;
pub mod tasks;

use std::sync::OnceLock;

use crate::transmission::TorrentAction;
use crate::tui::components::{Component, ComponentAction};
use crate::tui::ctx::CTX;

use popups::details::DetailsPopup;
use popups::stats::StatisticsPopup;
use ratatui::{
    prelude::*,
    widgets::{Cell, Row, Table},
};

use rm_config::CONFIG;
use rm_shared::{
    action::{Action, ErrorMessage, UpdateAction},
    current_window::TorrentWindow,
    status_task::StatusTask,
};
use rustmission_torrent::RustmissionTorrent;
use tasks::TorrentSelection;
use transmission_rpc::types::{Id, SessionGet, TorrentStatus};

use crate::transmission;

use self::bottom_stats::BottomStats;
use self::popups::files::FilesPopup;
use self::popups::{CurrentPopup, PopupManager};
use self::table_manager::TableManager;
use self::task_manager::TaskManager;

pub static SESSION_GET: OnceLock<SessionGet> = OnceLock::new();

pub struct TorrentsTab {
    pub current_window: TorrentWindow,
    table_manager: TableManager,
    popup_manager: PopupManager,
    task_manager: TaskManager,
    bottom_stats: BottomStats,
}

impl TorrentsTab {
    pub fn new() -> Self {
        let table_manager = TableManager::new();
        let bottom_stats = BottomStats::new();

        tokio::spawn(transmission::fetchers::stats());
        tokio::spawn(transmission::fetchers::torrents());
        tokio::spawn(transmission::fetchers::free_space());

        Self {
            bottom_stats,
            task_manager: TaskManager::new(),
            table_manager,
            popup_manager: PopupManager::new(),
            current_window: TorrentWindow::General,
        }
    }
}

impl Component for TorrentsTab {
    fn render(&mut self, f: &mut Frame, rect: Rect) {
        let [torrents_list_rect, stats_rect] =
            Layout::vertical([Constraint::Min(10), Constraint::Length(1)]).areas(rect);

        self.render_table(f, torrents_list_rect);
        self.bottom_stats.render(f, stats_rect);
        self.task_manager.render(f, stats_rect);
        self.popup_manager.render(f, f.area());
    }

    fn handle_actions(&mut self, action: Action) -> ComponentAction {
        use Action as A;

        if self.popup_manager.is_showing_popup() {
            self.popup_manager.handle_actions(action);
            return ComponentAction::Nothing;
        }

        if self.table_manager.sorting_is_being_selected {
            match action {
                A::Close => {
                    self.table_manager.leave_sorting();
                    self.task_manager.default();
                    CTX.send_action(Action::Render);
                }
                A::MoveToColumnLeft => {
                    self.table_manager.move_to_column_left();
                    CTX.send_action(Action::Render);
                }
                A::MoveToColumnRight => {
                    self.table_manager.move_to_column_right();
                    CTX.send_action(Action::Render);
                }
                A::Down | A::Up => {
                    self.table_manager.reverse_sort();
                    CTX.send_action(Action::Render);
                }
                A::Confirm => {
                    self.table_manager.apply_sort();
                    self.task_manager.default();
                    CTX.send_action(Action::Render);
                }
                _ => (),
            }
            return ComponentAction::Nothing;
        }

        if !self.table_manager.selected_torrents_ids.is_empty() && action.is_soft_quit() {
            self.table_manager
                .table
                .items
                .iter_mut()
                .for_each(|t| t.is_selected = false);
            self.table_manager.selected_torrents_ids.drain(..);
            self.task_manager.default();
            CTX.send_action(Action::Render);
            return ComponentAction::Nothing;
        }

        if action.is_quit() {
            CTX.send_action(Action::HardQuit);
            return ComponentAction::Nothing;
        }

        match action {
            A::Up => self.previous_torrent(),
            A::Down => self.next_torrent(),
            A::ScrollUpPage => self.scroll_page_up(),
            A::ScrollDownPage => self.scroll_page_down(),
            A::ScrollUpBy(amount) => self.scroll_up_by(amount),
            A::ScrollDownBy(amount) => self.scroll_down_by(amount),
            A::Home => self.select_first(),
            A::End => self.select_last(),
            A::ShowStats => self.show_statistics_popup(),
            A::ShowFiles => self.show_files_popup(),
            A::Confirm => self.show_details_popup(),
            A::Select => {
                self.table_manager.select_current_torrent();
                if !self.table_manager.selected_torrents_ids.is_empty() {
                    self.task_manager
                        .select(self.table_manager.selected_torrents_ids.len());
                } else {
                    self.task_manager.default();
                }
                CTX.send_action(Action::Render);
            }
            A::Pause => self.pause_current_torrent(),
            A::Delete => {
                if let Some(torrent_selection) = self.get_currently_selected() {
                    self.task_manager.delete_torrents(torrent_selection);
                }
            }
            A::Rename => {
                if let Some(TorrentSelection::Single(id, curr_name)) = self.get_currently_selected()
                {
                    self.task_manager.rename(id, curr_name);
                }
            }
            A::AddMagnet => self.task_manager.add_magnet(),
            A::Search => self.task_manager.search(
                &self
                    .table_manager
                    .filter
                    .as_ref()
                    .map(|f| f.pattern.clone()),
            ),
            A::MoveTorrent => {
                if let Some(selection) = self.get_currently_selected() {
                    if let Some(session_info) = SESSION_GET.get() {
                        self.task_manager
                            .move_torrent(selection, session_info.download_dir.clone());
                    }
                }
            }
            A::ChangeCategory => {
                if let Some(selection) = self.get_currently_selected() {
                    self.task_manager.change_category(selection);
                }
            }
            A::XdgOpen => self.xdg_open_current_torrent(),
            A::MoveToColumnLeft | A::MoveToColumnRight => {
                self.table_manager.enter_sorting_selection();
                self.task_manager.sort();
                CTX.send_action(Action::Render);
            }
            other => {
                self.task_manager.handle_actions(other);
            }
        };

        ComponentAction::Nothing
    }

    fn handle_update_action(&mut self, action: UpdateAction) {
        match action {
            UpdateAction::ChangeTorrentWindow(window) => {
                self.current_window = window;
            }
            UpdateAction::SessionStats(stats) => {
                if let Some(CurrentPopup::Stats(popup)) = &mut self.popup_manager.current_popup {
                    popup.update_stats(&stats)
                }
                self.bottom_stats.set_stats(stats);
            }
            UpdateAction::FreeSpace(free_space) => {
                self.bottom_stats.set_free_space(free_space);
            }
            UpdateAction::SearchFilterApply(filter) => {
                self.table_manager.set_filter(filter);
                self.table_manager.table.state.borrow_mut().select(Some(0));
                self.table_manager.update_rows_number();
                self.bottom_stats
                    .update_selected_indicator(&self.table_manager);
            }
            UpdateAction::SearchFilterClear => {
                self.table_manager.filter = None;
                self.table_manager.table.state.borrow_mut().select(Some(0));
                self.table_manager.update_rows_number();
                self.bottom_stats
                    .update_selected_indicator(&self.table_manager);
            }
            UpdateAction::UpdateTorrents(torrents) => {
                let torrents = torrents.into_iter().map(RustmissionTorrent::from).collect();

                self.table_manager.set_new_rows(torrents);
                if self.table_manager.selected_torrents_ids.is_empty()
                    && self.task_manager.is_selection_task()
                {
                    self.task_manager.default()
                }

                self.bottom_stats
                    .update_selected_indicator(&self.table_manager);
            }
            UpdateAction::UpdateCurrentTorrent(_) => {
                self.popup_manager.handle_update_action(action)
            }
            UpdateAction::CancelTorrentTask => {
                if self.task_manager.is_status_task_in_progress() {
                    return;
                }

                if !self.table_manager.selected_torrents_ids.is_empty() {
                    self.task_manager
                        .select(self.table_manager.selected_torrents_ids.len());
                } else {
                    self.task_manager.default();
                }
                CTX.send_update_action(UpdateAction::SwitchToNormalMode);
            }
            other => self.task_manager.handle_update_action(other),
        }
    }

    fn tick(&mut self) {
        self.task_manager.tick();
    }
}

impl TorrentsTab {
    fn render_table(&mut self, f: &mut Frame, rect: Rect) {
        let offset = self.table_manager.table.state.borrow().offset();
        let mut torrents_displaying_no = 0;
        let mut space_left = rect.height;
        for torrent in self.table_manager.table.items.iter().skip(offset) {
            if space_left == 0 {
                break;
            }

            if torrent.error.is_some() {
                space_left = space_left.saturating_sub(2);
            } else {
                space_left -= 1;
            }

            torrents_displaying_no += 1;
        }
        self.table_manager.torrents_displaying_no = torrents_displaying_no;

        let highlight_table_style = Style::default()
            .on_black()
            .bold()
            .fg(CONFIG.general.accent_color);

        let rows = self.table_manager.rows();

        let mut text_headers = self
            .table_manager
            .headers()
            .iter()
            .map(|h| h.header_name())
            .collect::<Vec<_>>();

        let sorted_header_name;
        if let Some(sort_header) = self.table_manager.sort_header {
            let icon = if self.table_manager.sort_reverse {
                &CONFIG.icons.sort_descending
            } else {
                &CONFIG.icons.sort_ascending
            };

            sorted_header_name = format!("{icon} {}", text_headers[sort_header]);
            text_headers[sort_header] = sorted_header_name.as_str();
        }

        let mut headers = text_headers
            .iter()
            .cloned()
            .map(Cell::from)
            .collect::<Vec<_>>();

        if let Some(sort_header) = self.table_manager.sort_header {
            if self.table_manager.sorting_is_being_selected {
                headers[sort_header] = headers[sort_header]
                    .clone()
                    .style(Style::default().fg(CONFIG.general.accent_color));
            }
        }

        let table_widget = {
            let table = Table::new(rows, &self.table_manager.widths)
                .row_highlight_style(highlight_table_style);
            if !CONFIG.general.headers_hide {
                table.header(Row::new(headers))
            } else {
                table
            }
        };

        f.render_stateful_widget(
            table_widget,
            rect,
            &mut self.table_manager.table.state.borrow_mut(),
        );
    }

    fn get_currently_selected(&mut self) -> Option<TorrentSelection> {
        if !self.table_manager.selected_torrents_ids.is_empty() {
            Some(TorrentSelection::Many(
                self.table_manager
                    .selected_torrents_ids
                    .clone()
                    .into_iter()
                    .map(Id::Id)
                    .collect(),
            ))
        } else if let Some(t) = self.table_manager.current_torrent() {
            Some(TorrentSelection::Single(
                t.id.clone(),
                t.torrent_name.to_string(),
            ))
        } else {
            None
        }
    }

    fn show_files_popup(&mut self) {
        if let Some(highlighted_torrent) = self.table_manager.current_torrent() {
            let popup = FilesPopup::new(highlighted_torrent.id.clone());
            self.popup_manager.show_popup(CurrentPopup::Files(popup));

            let update_action = UpdateAction::ChangeTorrentWindow(TorrentWindow::FileViewer);
            CTX.send_update_action(update_action);
        }
    }

    fn show_details_popup(&mut self) {
        if let Some(highlighted_torrent) = self.table_manager.current_torrent() {
            let popup = DetailsPopup::new(highlighted_torrent.clone());
            self.popup_manager.show_popup(CurrentPopup::Details(popup));
            CTX.send_action(Action::Render);
        }
    }

    fn show_statistics_popup(&mut self) {
        if let Some(stats) = &self.bottom_stats.stats {
            let popup = StatisticsPopup::new(stats.clone());
            self.popup_manager.show_popup(CurrentPopup::Stats(popup));
            CTX.send_action(Action::Render)
        }
    }

    fn previous_torrent(&mut self) {
        self.table_manager.table.previous();
        self.bottom_stats
            .update_selected_indicator(&self.table_manager);
        CTX.send_action(Action::Render);
    }

    fn next_torrent(&mut self) {
        self.table_manager.table.next();
        self.bottom_stats
            .update_selected_indicator(&self.table_manager);
        CTX.send_action(Action::Render);
    }

    fn scroll_up_by(&mut self, amount: u8) {
        self.table_manager.table.scroll_up_by(usize::from(amount));
        self.bottom_stats
            .update_selected_indicator(&self.table_manager);
        CTX.send_action(Action::Render);
    }

    fn scroll_down_by(&mut self, amount: u8) {
        self.table_manager.table.scroll_down_by(usize::from(amount));
        self.bottom_stats
            .update_selected_indicator(&self.table_manager);
        CTX.send_action(Action::Render);
    }

    fn scroll_page_down(&mut self) {
        let scroll_by = self.table_manager.torrents_displaying_no;
        self.table_manager.table.scroll_down_by(scroll_by as usize);
        self.bottom_stats
            .update_selected_indicator(&self.table_manager);
        CTX.send_action(Action::Render);
    }

    fn scroll_page_up(&mut self) {
        let scroll_by = self.table_manager.torrents_displaying_no;
        self.table_manager.table.scroll_up_by(scroll_by as usize);
        self.bottom_stats
            .update_selected_indicator(&self.table_manager);
        CTX.send_action(Action::Render);
    }

    fn select_first(&mut self) {
        self.table_manager.table.select_first();
        self.bottom_stats
            .update_selected_indicator(&self.table_manager);
        CTX.send_action(Action::Render);
    }

    fn select_last(&mut self) {
        self.table_manager.table.select_last();
        self.bottom_stats
            .update_selected_indicator(&self.table_manager);
        CTX.send_action(Action::Render);
    }

    fn pause_current_torrent(&mut self) {
        if let Some(torrent) = self.table_manager.current_torrent() {
            let torrent_id = torrent.id.clone();
            match torrent.status() {
                TorrentStatus::Stopped => {
                    CTX.send_torrent_action(TorrentAction::Start(vec![torrent_id]));
                    torrent.update_status(TorrentStatus::Downloading);
                    CTX.send_action(Action::Render);
                }
                _ => {
                    CTX.send_torrent_action(TorrentAction::Stop(vec![torrent_id]));
                    torrent.update_status(TorrentStatus::Stopped);
                    CTX.send_action(Action::Render);
                }
            }
        }
    }

    fn xdg_open_current_torrent(&mut self) {
        if let Some(torrent) = self.table_manager.current_torrent() {
            let torrent_location = torrent.torrent_location();
            match open::that_detached(&torrent_location) {
                Ok(()) => CTX.send_update_action(UpdateAction::StatusTaskSetSuccess(
                    StatusTask::new_open(torrent_location),
                )),
                Err(err) => {
                    let desc = format!(
                        "Encountered an error while trying to open \"{}\"",
                        torrent_location
                    );
                    let err_msg = ErrorMessage::new(
                        "Failed to open a torrent directory",
                        desc,
                        Box::new(err),
                    );
                    CTX.send_update_action(UpdateAction::Error(Box::new(err_msg)));
                }
            };
        }
    }
}
