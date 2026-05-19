use std::{collections::HashMap, fs::read_to_string};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, HighlightSpacing, List, ListState},
};
use serde::Deserialize;

use crate::{action::Action, components::Component};

#[derive(Deserialize, Debug, Clone)]
struct Message {
    #[serde(rename = "m")]
    m_id: String,
    #[serde(rename = "dt")]
    date_time: String,
    #[serde(rename = "refs")]
    refs: Option<Vec<String>>,
    #[serde(rename = "s")]
    subject: String,
    #[serde(rename = "f")]
    from: Vec<Vec<Option<String>>>,
}

struct Node {
    m_id: String,
    last_updated: String,
}

#[derive(PartialEq)]
enum LocalMode {
    Idle,
    Listing,
    Thread,
}

pub struct Patchsets {
    local_mode: LocalMode,
    messages: Vec<Message>,
    map_id_message: HashMap<String, Message>,
    map_id_children: HashMap<String, Vec<String>>,
    roots: Vec<Node>,
    index: usize,
    thread: Vec<String>,
}

impl Patchsets {
    pub fn new() -> Self {
        Self {
            local_mode: LocalMode::Idle,
            messages: Vec::new(),
            map_id_message: HashMap::new(),
            map_id_children: HashMap::new(),
            roots: Vec::new(),
            index: 0,
            thread: Vec::new(),
        }
    }

    fn prepare_list(&mut self, json_path_str: String) -> color_eyre::Result<()> {
        let data = read_to_string(&json_path_str)?;

        let raw_values: Vec<serde_json::Value> = serde_json::from_str(&data)?;

        for val in raw_values {
            if val.is_object() {
                if let Ok(message) = serde_json::from_value::<Message>(val) {
                    self.messages.push(message);
                }
            }
        }

        for message in &self.messages {
            self.map_id_message
                .insert(message.m_id.clone(), message.clone());
            if let Some(refs) = message.refs.clone() {
                let parent_m_id = refs.last().unwrap().to_owned();
                self.map_id_children
                    .entry(parent_m_id)
                    .or_default()
                    .push(message.m_id.clone());
            } else {
                let root = Node {
                    m_id: message.m_id.clone(),
                    last_updated: String::new(),
                };
                self.roots.push(root);
            }
        }

        for children in self.map_id_children.values_mut() {
            children.sort_unstable_by(|a_m_id, b_m_id| {
                let a_datetime = self
                    .map_id_message
                    .get(a_m_id)
                    .map(|message| message.date_time.as_str())
                    .unwrap_or("");
                let b_datetime = self
                    .map_id_message
                    .get(b_m_id)
                    .map(|m| m.date_time.as_str())
                    .unwrap_or("");

                // Compare strings directly since they are ISO 8601
                // This sorts them descending (latest replies first).
                b_datetime.cmp(a_datetime)
            });
        }

        for root in &mut self.roots {
            let mut max_ts = String::new();
            // Use a simple vector as a stack for an iterative DFS
            let mut stack = vec![root.m_id.clone()];

            while let Some(current_m_id) = stack.pop() {
                if let Some(message) = self.map_id_message.get(&current_m_id) {
                    // String comparison works perfectly for ISO 8601
                    if message.date_time > max_ts {
                        max_ts = message.date_time.clone();
                    }
                }

                // If this message has replies, push them to the stack to be processed
                if let Some(children) = self.map_id_children.get(&current_m_id) {
                    stack.extend(children.iter().cloned());
                }
            }

            // Update the root with the highest timestamp found in its thread tree
            root.last_updated = max_ts;
        }

        self.roots
            .sort_unstable_by(|a, b| b.last_updated.cmp(&a.last_updated));

        self.local_mode = LocalMode::Listing;
        Ok(())
    }

    fn open_thread(&mut self) {
        let root_m_id = self.roots.get(self.index).unwrap().m_id.clone();
        self.index = 0;

        let mut stack = vec![(0, root_m_id)];

        while let Some((i, current_m_id)) = stack.pop() {
            if let Some(message) = self.map_id_message.get(&current_m_id) {
                let line = if i != 0 {
                    format!("{}\u{21B3} {}", "  ".repeat(i), &message.subject)
                } else {
                    message.subject.clone()
                };
                self.thread.push(line);
            }

            // If this message has replies, push them to the stack to be processed
            if let Some(children) = self.map_id_children.get(&current_m_id) {
                stack.extend(children.iter().map(|c| (i + 1, c.clone())));
            }
        }

        self.local_mode = LocalMode::Thread;
    }
}

impl Component for Patchsets {
    fn handle_key_event(&mut self, key: KeyEvent) -> color_eyre::Result<Option<Action>> {
        let action = match self.local_mode {
            LocalMode::Listing | LocalMode::Thread => match key.code {
                KeyCode::Char('j') => Some(Action::PatchsetsAddIndex),
                KeyCode::Char('k') => Some(Action::PatchsetsSubIndex),
                KeyCode::Enter => Some(Action::PatchsetsThread),
                _ => None,
            },
            _ => None,
        };

        Ok(action)
    }

    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        match action {
            Action::PatchsetsList(json_path_str) => self.prepare_list(json_path_str)?,
            Action::PatchsetsAddIndex => match self.local_mode {
                LocalMode::Listing => {
                    if self.index < self.roots.len() - 1 {
                        self.index = self.index + 1;
                    }
                }
                LocalMode::Thread => {
                    if self.index < self.thread.len() - 1 {
                        self.index = self.index + 1;
                    }
                }
                _ => {}
            },
            Action::PatchsetsSubIndex => self.index = self.index.saturating_sub(1),
            Action::PatchsetsThread => self.open_thread(),
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        if self.local_mode == LocalMode::Listing {
            let rects = Layout::default()
                .constraints(
                    [
                        Constraint::Min(1),
                        Constraint::Percentage(100),
                        Constraint::Min(4),
                    ]
                    .as_ref(),
                )
                .split(area);
            let rects = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(
                    [
                        Constraint::Min(1),
                        Constraint::Percentage(100),
                        Constraint::Min(1),
                    ]
                    .as_ref(),
                )
                .split(rects[1]);

            let list_items: Vec<String> = self
                .roots
                .iter()
                .map(|root| self.map_id_message.get(&root.m_id).unwrap().subject.clone())
                .collect();
            let list_block = Block::default().borders(Borders::NONE);
            let list = List::new(list_items)
                .block(list_block)
                .highlight_style(Style::default().fg(Color::Black).bg(Color::LightYellow))
                .highlight_symbol(">")
                .highlight_spacing(HighlightSpacing::Always);

            let mut list_state = ListState::default();
            list_state.select(Some(self.index));

            frame.render_stateful_widget(list, rects[1], &mut list_state);
        } else if self.local_mode == LocalMode::Thread {
            let rects = Layout::default()
                .constraints(
                    [
                        Constraint::Min(1),
                        Constraint::Percentage(100),
                        Constraint::Min(4),
                    ]
                    .as_ref(),
                )
                .split(area);
            let rects = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(
                    [
                        Constraint::Min(1),
                        Constraint::Percentage(100),
                        Constraint::Min(1),
                    ]
                    .as_ref(),
                )
                .split(rects[1]);

            let list_items: Vec<String> = self.thread.clone();
            let list_block = Block::default().borders(Borders::NONE);
            let list = List::new(list_items)
                .block(list_block)
                .highlight_style(Style::default().fg(Color::Black).bg(Color::LightYellow))
                .highlight_symbol(">")
                .highlight_spacing(HighlightSpacing::Always);

            let mut list_state = ListState::default();
            list_state.select(Some(self.index));

            frame.render_stateful_widget(list, rects[1], &mut list_state);
        }
        Ok(())
    }
}
