use std::{fs::File, process::Stdio};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, Paragraph},
};
use serde::{Deserialize, Serialize};
use strum::Display;
use tokio::{process::Command, sync::mpsc::UnboundedSender};
use tracing::{error, info};

use crate::{action::Action, components::Component, config::Config};

const MAX_NOTIFICATION_TICKS: usize = 8;
const MAX_SPINNER: usize = 4;
const SPINNER: [&str; MAX_SPINNER] = ["", ".", "..", "..."];
const LAST_MONTH_QUERY: &str = "((s:patch OR s:rfc) AND NOT s:re:) AND rt:1.month.ago..";

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum LocalMode {
    Idle,
    Processing,
    Creating,
    Updating,
    Fetching,
    ExitingProcessing,
}

pub struct Lei {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    local_mode: LocalMode,
    spinner: usize,
    notification_ticks: usize,
}

impl Lei {
    pub fn new() -> Self {
        Self {
            command_tx: None,
            config: Config::default(),
            local_mode: LocalMode::Idle,
            spinner: 0,
            notification_ticks: 0,
        }
    }

    fn update_local_inbox(&self, lore_url: String, list: String, query: String) {
        let tx = self.command_tx.clone().unwrap();
        let mut inbox_dir = self.config.config.data_dir.clone();
        inbox_dir.push(list.clone());
        tokio::spawn(async move {
            tx.send(Action::LeiEnterProcessing).unwrap();
            let inbox_dir_str = inbox_dir.to_str().unwrap();

            // Check if local Public-Inbox already exists and create one, otherwise
            if !inbox_dir.exists() {
                info!("creating public inbox {inbox_dir_str}");
                tx.send(Action::LeiSetMode(LocalMode::Creating)).unwrap();
                if let Ok(exit_status) = Command::new("lei")
                    .arg("q")
                    .arg(format!("--only={lore_url}/{list}/"))
                    .arg(format!("--output=v2:{inbox_dir_str}"))
                    .arg("--threads")
                    .arg("--dedupe=mid")
                    .arg(query)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await
                {
                    if !exit_status.success() {
                        error!("lei command exit status was unsuccessful {exit_status:?}");
                    }
                } else {
                    error!("failed to execute command");
                }
            }

            // Update Public Inbox
            info!("updating public inbox {inbox_dir_str}");
            tx.send(Action::LeiSetMode(LocalMode::Updating)).unwrap();
            if let Ok(exit_status) = Command::new("lei")
                .arg("up")
                .arg(inbox_dir_str)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await
            {
                if !exit_status.success() {
                    error!("lei command exit status was unsuccessful {exit_status:?}");
                }
            } else {
                error!("failed to execute command");
            }

            tx.send(Action::LeiExitProcessing).unwrap();
        });
    }

    fn fetch_patchsets(&self, list: String, query: String) {
        let tx = self.command_tx.clone().unwrap();
        let mut data_dir = self.config.config.data_dir.clone();
        tokio::spawn(async move {
            tx.send(Action::LeiEnterProcessing).unwrap();

            data_dir.push(format!("{list}"));
            let inbox_dir_str = data_dir.to_str().unwrap();
            let mut data_dir = data_dir.clone();
            data_dir.pop();
            data_dir.push(format!("{list}.json"));
            let json_path_str = data_dir.to_str().unwrap();

            // Update Public Inbox
            info!("fetching patchsets from {inbox_dir_str}");
            tx.send(Action::LeiSetMode(LocalMode::Fetching)).unwrap();
            if let Ok(output) = File::create(&json_path_str) {
                if let Ok(exit_status) = Command::new("lei")
                    .arg("q")
                    .arg(format!("--only={inbox_dir_str}"))
                    .arg("--no-local")
                    .arg("--threads")
                    .arg("--dedupe=mid")
                    .arg(query)
                    .stdout(output)
                    .stderr(Stdio::null())
                    .status()
                    .await
                {
                    if !exit_status.success() {
                        error!("lei command exit status was unsuccessful {exit_status:?}");
                    }
                } else {
                    error!("failed to execute command");
                }
            } else {
                error!("failed to create {json_path_str}");
            }

            tx.send(Action::LeiExitProcessing).unwrap();
        });
    }
}

impl Component for Lei {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> color_eyre::Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> color_eyre::Result<Option<Action>> {
        let action = match self.local_mode {
            LocalMode::Idle => match key.code {
                KeyCode::Char('r') => Some(Action::LeiUpdateInbox),
                KeyCode::Char('f') => Some(Action::LeiFetchPatchsets),
                _ => None,
            },
            _ => None,
        };

        Ok(action)
    }

    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        match action {
            Action::Tick => {
                if self.local_mode == LocalMode::ExitingProcessing {
                    self.notification_ticks += 1;
                    if self.notification_ticks >= MAX_NOTIFICATION_TICKS {
                        self.notification_ticks = 0;
                        return Ok(Some(Action::LeiEnterIdle));
                    }
                } else if self.local_mode != LocalMode::Idle {
                    self.spinner += 1;
                    if self.spinner >= MAX_SPINNER {
                        self.spinner = 0;
                    }
                }
            }
            Action::LeiSetMode(local_mode) => self.local_mode = local_mode,
            Action::LeiUpdateInbox => self.update_local_inbox(
                "https://lore.kernel.org".to_string(),
                "amd-gfx".to_string(),
                LAST_MONTH_QUERY.to_string(),
            ),
            Action::LeiFetchPatchsets => self.fetch_patchsets(
                "amd-gfx".to_string(),
                LAST_MONTH_QUERY.to_string(),
            ),
            Action::LeiEnterProcessing => {
                self.spinner = 0;
                self.local_mode = LocalMode::Processing;
            }
            Action::LeiExitProcessing => {
                self.notification_ticks = 0;
                self.local_mode = LocalMode::ExitingProcessing;
            }
            Action::LeiEnterIdle => {
                self.local_mode = LocalMode::Idle;
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        if self.local_mode != LocalMode::Idle {
            let rects = Layout::default()
                .constraints([Constraint::Percentage(100), Constraint::Min(3)].as_ref())
                .split(area);
            let rects = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(70), Constraint::Min(0)].as_ref())
                .split(rects[1]);

            let text = match self.local_mode {
                LocalMode::Idle => "Idle".to_string(),
                LocalMode::Processing => format!("Processing{}", SPINNER[self.spinner]),
                LocalMode::Creating => format!("Creating inbox{}", SPINNER[self.spinner]),
                LocalMode::Updating => format!("Updating inbox{}", SPINNER[self.spinner]),
                LocalMode::Fetching => format!("Fetching patchsets{}", SPINNER[self.spinner]),
                LocalMode::ExitingProcessing => "Finished processing!".to_string(),
            };
            let style = match self.local_mode {
                LocalMode::Idle | LocalMode::ExitingProcessing => Style::default().fg(Color::Cyan),
                LocalMode::Processing | LocalMode::Creating | LocalMode::Updating | LocalMode::Fetching => {
                    Style::default().fg(Color::Yellow)
                }
            };
            frame.render_widget(
                Paragraph::new(text)
                    .block(
                        Block::default()
                            .title(" lei status ")
                            .title_alignment(Alignment::Center)
                            .borders(Borders::ALL)
                            .border_style(style)
                            .border_type(BorderType::Rounded),
                    )
                    .style(style)
                    .alignment(Alignment::Center),
                rects[1],
            );
        }
        Ok(())
    }
}
