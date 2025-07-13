use std::{collections::BTreeMap, time::Duration};

use ratatui::{
    prelude::*,
    style::Styled,
    widgets::{Clear, List, ListState, Paragraph},
};
use rm_config::{
    keymap::{actions::torrents_tab_file_viewer::TorrentsFileViewerAction, GeneralAction},
    CONFIG,
};
use tokio::{sync::oneshot, task::JoinHandle};
use transmission_rpc::types::{Id, Priority, Torrent, TorrentSetArgs};
use tui_tree_widget::{Tree, TreeItem, TreeState};

use crate::{
    transmission::TorrentAction,
    tui::{
        components::{
            keybinding_style, popup_block, popup_close_button, popup_close_button_highlight,
            popup_rects, Component, ComponentAction,
        },
        ctx::CTX,
    },
};
use rm_shared::{
    action::{Action, ErrorMessage, UpdateAction},
    status_task::StatusTask,
    utils::{bytes_to_human_format, bytes_to_short_human_format},
};

struct PriorityPopup {
    torrent_id: Id,
    files: Vec<usize>,
    list_state: ListState,
}

impl PriorityPopup {
    fn new(torrent_id: Id, files: Vec<usize>) -> Self {
        Self {
            torrent_id,
            files,
            list_state: ListState::default().with_selected(Some(1)),
        }
    }
}

impl Component for PriorityPopup {
    fn handle_actions(&mut self, action: Action) -> ComponentAction {
        if action.is_soft_quit() {
            return ComponentAction::Quit;
        }

        match action {
            Action::Up => {
                self.list_state.select_previous();
                CTX.send_action(Action::Render);
                return ComponentAction::Nothing;
            }
            Action::Down => {
                self.list_state.select_next();
                CTX.send_action(Action::Render);
                return ComponentAction::Nothing;
            }
            Action::Confirm => {
                let torrent_id = match self.torrent_id {
                    Id::Id(id) => id,
                    Id::Hash(_) => unreachable!(),
                };

                let args = match self.list_state.selected().unwrap() {
                    1 => {
                        let args = TorrentSetArgs::new().priority_high(self.files.clone());
                        CTX.send_torrent_action(TorrentAction::SetArgs(
                            Box::new(args),
                            Some(vec![self.torrent_id.clone()]),
                        ));
                    }
                    _ => unreachable!(),
                };
                CTX.send_action(Action::Render);
                return ComponentAction::Quit;
            }
            _ => return ComponentAction::Nothing,
        }
    }

    fn handle_update_action(&mut self, action: UpdateAction) {
        let _action = action;
    }

    fn render(&mut self, f: &mut Frame, rect: Rect) {
        let [block_rect] = Layout::horizontal([Constraint::Length(20)])
            .flex(layout::Flex::Center)
            .areas(rect);
        let [block_rect] = Layout::vertical([Constraint::Length(5)])
            .flex(layout::Flex::Center)
            .areas(block_rect);

        let block = popup_block(" Priority ");

        let list_rect = block_rect.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        let list = List::new([
            Text::raw("Low").centered(),
            Text::raw("Normal").centered(),
            Text::raw("High").centered(),
        ])
        .highlight_style(
            Style::default()
                .fg(CONFIG.general.accent_color)
                .bg(Color::Black)
                .bold(),
        );

        f.render_widget(block, block_rect);
        f.render_stateful_widget(list, list_rect, &mut self.list_state);
    }
}

pub struct FilesPopup {
    torrent: Option<Torrent>,
    torrent_id: Id,
    priority_popup: Option<PriorityPopup>,
    tree_state: TreeState<String>,
    tree: Node,
    current_focus: CurrentFocus,
    switched_after_fetched_data: bool,
    torrent_info_task_handle: JoinHandle<()>,
}

async fn fetch_new_files(torrent_id: Id) {
    loop {
        let (torrent_tx, torrent_rx) = oneshot::channel();
        CTX.send_torrent_action(TorrentAction::GetTorrentsById(
            vec![torrent_id.clone()],
            torrent_tx,
        ));

        match torrent_rx.await.unwrap() {
            Ok(mut torrents) => {
                CTX.send_update_action(UpdateAction::UpdateCurrentTorrent(Box::new(
                    torrents.pop().unwrap(),
                )));
            }
            Err(err_message) => {
                CTX.send_update_action(UpdateAction::Error(err_message));
            }
        };

        tokio::time::sleep(Duration::from_secs(6)).await;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CurrentFocus {
    CloseButton,
    Files,
}

impl FilesPopup {
    pub fn new(torrent_id: Id) -> Self {
        let torrent = None;
        let tree_state = TreeState::default();
        let tree = Node::new();

        let torrent_info_task_handle = tokio::task::spawn(fetch_new_files(torrent_id.clone()));

        Self {
            torrent,
            tree_state,
            tree,
            current_focus: CurrentFocus::CloseButton,
            switched_after_fetched_data: false,
            torrent_id,
            torrent_info_task_handle,
            priority_popup: None,
        }
    }

    fn switch_focus(&mut self) {
        match self.current_focus {
            CurrentFocus::CloseButton => self.current_focus = CurrentFocus::Files,
            CurrentFocus::Files => self.current_focus = CurrentFocus::CloseButton,
        }
    }

    fn selected_ids(&self) -> Vec<usize> {
        self.tree_state
            .selected()
            .iter()
            .filter_map(|str_id| str_id.parse::<usize>().ok())
            .collect()
    }
}

impl Component for FilesPopup {
    #[must_use]
    fn handle_actions(&mut self, action: Action) -> ComponentAction {
        use Action as A;

        match (&mut self.priority_popup, action, self.current_focus) {
            (Some(priority_popup), action, _) => {
                if priority_popup.handle_actions(action).is_quit() {
                    self.priority_popup = None;
                    CTX.send_action(A::Render);
                    return ComponentAction::Nothing;
                }
            }
            (_, action, _) if action.is_soft_quit() => {
                self.torrent_info_task_handle.abort();
                return ComponentAction::Quit;
            }
            (None, A::ChangeFilePriority, CurrentFocus::Files) => {
                self.priority_popup = Some(PriorityPopup::new(
                    self.torrent_id.clone(),
                    self.selected_ids(),
                ));
                CTX.send_action(A::Render);
            }
            (None, A::ChangeFocus, _) => {
                self.switch_focus();
                CTX.send_action(A::Render);
            }
            (None, A::Confirm, CurrentFocus::CloseButton) => {
                self.torrent_info_task_handle.abort();
                return ComponentAction::Quit;
            }
            (None, A::Select | A::Confirm, CurrentFocus::Files) => {
                if self.torrent.is_some() {
                    let mut wanted_ids = self
                        .torrent
                        .as_ref()
                        .unwrap()
                        .wanted
                        .as_ref()
                        .unwrap()
                        .clone();

                    let selected_ids = self.selected_ids();

                    if selected_ids.is_empty() {
                        self.tree_state.toggle_selected();
                        CTX.send_action(A::Render);
                        return ComponentAction::Nothing;
                    }

                    let mut wanted_in_selection_no = 0;
                    for selected_id in &selected_ids {
                        if wanted_ids[*selected_id as usize] {
                            wanted_in_selection_no += 1;
                        } else {
                            wanted_in_selection_no -= 1;
                        }
                    }

                    if wanted_in_selection_no > 0 {
                        for selected_id in &selected_ids {
                            wanted_ids[*selected_id as usize] = false;
                        }
                    } else {
                        for selected_id in &selected_ids {
                            wanted_ids[*selected_id as usize] = true;
                        }
                    }

                    let args = {
                        if wanted_in_selection_no > 0 {
                            for transmission_file in self.tree.get_by_ids(&selected_ids) {
                                transmission_file.set_wanted(false);
                            }
                            TorrentSetArgs::default().files_unwanted(selected_ids)
                        } else {
                            for transmission_file in self.tree.get_by_ids(&selected_ids) {
                                transmission_file.set_wanted(true);
                            }
                            TorrentSetArgs::default().files_wanted(selected_ids)
                        }
                    };

                    CTX.send_torrent_action(TorrentAction::SetArgs(
                        Box::new(args),
                        Some(vec![self.torrent_id.clone()]),
                    ));

                    CTX.send_action(Action::Render);
                }
            }

            (None, A::Up | A::ScrollUpBy(_), CurrentFocus::Files) => {
                self.tree_state.key_up();
                CTX.send_action(Action::Render);
            }
            (None, A::Down | A::ScrollDownBy(_), CurrentFocus::Files) => {
                self.tree_state.key_down();
                CTX.send_action(Action::Render);
            }
            (None, A::XdgOpen, CurrentFocus::Files) => {
                if let Some(torrent) = &self.torrent {
                    let mut identifier = self.tree_state.selected().to_vec();

                    if identifier.is_empty() {
                        return ComponentAction::Nothing;
                    }

                    if let Ok(file_id) = identifier.last().unwrap().parse::<usize>() {
                        identifier.pop();
                        identifier
                            .push(self.tree.get_by_ids(&[file_id]).pop().unwrap().name.clone())
                    }

                    let sub_path = identifier.join("/");

                    let path = format!("{}/{}", torrent.download_dir.as_ref().unwrap(), sub_path,);

                    match open::that_detached(&path) {
                        Ok(()) => CTX.send_update_action(UpdateAction::StatusTaskSetSuccess(
                            StatusTask::new_open(&path),
                        )),
                        Err(err) => {
                            let desc =
                                format!("An error occured while trying to open \"{}\"", path);
                            let err_msg =
                                ErrorMessage::new("Failed to open a file", desc, Box::new(err));
                            CTX.send_update_action(UpdateAction::Error(Box::new(err_msg)));
                        }
                    };
                }
            }

            _ => (),
        }
        ComponentAction::Nothing
    }

    fn handle_update_action(&mut self, action: UpdateAction) {
        if let UpdateAction::UpdateCurrentTorrent(torrent) = action {
            let new_tree = Node::new_from_torrent(&torrent);
            self.torrent = Some(*torrent);
            self.tree = new_tree;
        }
    }

    fn render(&mut self, f: &mut Frame, rect: Rect) {
        let (popup_rect, block_rect, text_rect) = popup_rects(rect, 75, 75);

        let highlight_style = Style::default().fg(CONFIG.general.accent_color);
        let bold_highlight_style = highlight_style.on_black().bold();

        let block = popup_block(" Files ");

        if self.tree_state.selected().is_empty() {
            self.tree_state.select_first();
        }

        if let Some(torrent) = &self.torrent {
            if !self.switched_after_fetched_data {
                self.current_focus = CurrentFocus::Files;
                self.switched_after_fetched_data = true;
            }

            let close_button = {
                match self.current_focus {
                    CurrentFocus::CloseButton => popup_close_button_highlight(),
                    CurrentFocus::Files => popup_close_button(),
                }
            };

            let tree_highlight_style = {
                if self.current_focus == CurrentFocus::Files {
                    bold_highlight_style
                } else {
                    Style::default()
                }
            };

            let download_dir = torrent.download_dir.as_ref().expect("Requested");

            let keybinding_tip = {
                if CONFIG.general.beginner_mode {
                    let mut keys = vec![];

                    if let Some(key) = CONFIG
                        .keybindings
                        .general
                        .get_keys_for_action_joined(GeneralAction::Select)
                    {
                        keys.push(Span::raw(" "));
                        keys.push(Span::styled(key, keybinding_style()));
                        keys.push(Span::raw(" - toggle | "));
                    }

                    if let Some(key) = CONFIG
                        .keybindings
                        .general
                        .get_keys_for_action_joined(GeneralAction::XdgOpen)
                    {
                        keys.push(Span::styled(key, keybinding_style()));
                        keys.push(Span::raw(" - xdg_open | "));
                    }

                    if let Some(key) = CONFIG
                        .keybindings
                        .torrents_tab_file_viewer
                        .get_keys_for_action_joined(TorrentsFileViewerAction::ChangeFilePriority)
                    {
                        keys.push(Span::styled(key, keybinding_style()));
                        keys.push(Span::raw(" - change file priority"));
                    }

                    Line::from(keys)
                } else {
                    Line::from("")
                }
            };

            let block = block
                .title_top(
                    format!(" {} ", download_dir)
                        .set_style(highlight_style)
                        .into_right_aligned_line(),
                )
                .title_bottom(close_button)
                .title_bottom(Line::from(keybinding_tip).left_aligned());

            let tree_items = self.tree.make_tree();

            let tree_widget = Tree::new(&tree_items)
                .unwrap()
                .block(block)
                .highlight_style(tree_highlight_style);

            f.render_widget(Clear, popup_rect);
            f.render_stateful_widget(tree_widget, block_rect, &mut self.tree_state);

            if let Some(popup) = &mut self.priority_popup {
                popup.render(f, rect);
            }
        } else {
            let paragraph = Paragraph::new("Loading...");
            let block = block.title(popup_close_button_highlight());
            f.render_widget(Clear, popup_rect);
            f.render_widget(paragraph, text_rect);
            f.render_widget(block, block_rect);
        }
    }
}

struct TransmissionFile {
    name: String,
    id: usize,
    wanted: bool,
    priority: Priority,
    length: i64,
    bytes_completed: i64,
}

impl TransmissionFile {
    fn set_wanted(&mut self, new_wanted: bool) {
        self.wanted = new_wanted;
    }

    fn priority_str(&self) -> &'static str {
        match self.priority {
            Priority::Low => "Low",
            Priority::Normal => "Normal",
            Priority::High => "High",
        }
    }
}

struct Node {
    items: Vec<TransmissionFile>,
    directories: BTreeMap<String, Node>,
}

impl Node {
    fn new() -> Self {
        Self {
            items: vec![],
            directories: BTreeMap::new(),
        }
    }

    fn new_from_torrent(torrent: &Torrent) -> Self {
        let files = torrent.files.as_ref().unwrap();
        let mut root = Self::new();

        for (id, file) in files.iter().enumerate() {
            let path: Vec<String> = file.name.split('/').map(str::to_string).collect();

            let wanted = torrent.wanted.as_ref().unwrap()[id] != false;

            let priority = torrent.priorities.as_ref().unwrap()[id].clone();

            let file = TransmissionFile {
                id,
                name: path[path.len() - 1].clone(),
                wanted,
                length: file.length,
                bytes_completed: file.bytes_completed,
                priority,
            };

            root.add_transmission_file(file, &path);
        }

        root
    }

    fn add_transmission_file(&mut self, file: TransmissionFile, remaining_path: &[String]) {
        if let Some((first, rest)) = remaining_path.split_first() {
            if rest.is_empty() {
                // We've found home for our TransmissionFile! :D
                self.items.push(file);
            } else {
                let child = self
                    .directories
                    .entry(first.to_string())
                    .or_insert_with(Self::new);
                child.add_transmission_file(file, rest);
            }
        }
    }

    fn get_by_ids(&mut self, ids: &[usize]) -> Vec<&mut TransmissionFile> {
        let mut transmission_files = vec![];
        for file in &mut self.items {
            if ids.contains(&(file.id as usize)) {
                transmission_files.push(file);
            }
        }
        for node in self.directories.values_mut() {
            transmission_files.extend(node.get_by_ids(ids))
        }
        transmission_files
    }

    fn make_tree(&self) -> Vec<TreeItem<String>> {
        let mut tree_items = vec![];
        for transmission_file in &self.items {
            let mut name = Line::default();
            let progress: f64 = if transmission_file.length != 0 {
                transmission_file.bytes_completed as f64 / transmission_file.length as f64
            } else {
                0.0
            };
            let mut progress_percent = format!("{}% ", (progress * 100f64).ceil());

            if progress_percent.len() == 3 {
                progress_percent.push(' ');
            }

            if transmission_file.wanted {
                name.push_span(Span::raw("󰄲 "));
            } else {
                name.push_span(Span::raw(" "));
            }

            name.push_span(Span::raw("| "));

            name.push_span(format!("[{}] ", transmission_file.priority_str()));

            if progress != 1.0 {
                name.push_span(Span::styled(
                    progress_percent,
                    Style::new().fg(CONFIG.general.accent_color),
                ));

                name.push_span(Span::raw("["));
                name.push_span(Span::styled(
                    bytes_to_short_human_format(transmission_file.bytes_completed),
                    Style::new().fg(CONFIG.general.accent_color),
                ));
                name.push_span(Span::raw("/"));
                name.push_span(Span::raw(bytes_to_short_human_format(
                    transmission_file.length,
                )));
                name.push_span(Span::raw("] "));
            } else {
                name.push_span(Span::raw("["));
                name.push_span(bytes_to_human_format(transmission_file.length));
                name.push_span(Span::raw("] "));
            }

            name.push_span(Span::raw(transmission_file.name.as_str()));

            tree_items.push(TreeItem::new_leaf(transmission_file.id.to_string(), name));
        }

        for (key, value) in &self.directories {
            tree_items.push(TreeItem::new(key.clone(), key.clone(), value.make_tree()).unwrap());
        }
        tree_items
    }
}
