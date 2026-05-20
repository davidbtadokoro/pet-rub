use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Frame, layout::Rect};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{action::Action, config::Config};

/// Set constants to not worry about resolving the target kernel tree and branch
const KERNEL_TREE_PATH: &str = "foo";
const BASE_BRANCH: &str = "bar";

/// Ktree (kernel tree)
///
/// The idea of the component is to represent the kernel tree entity. The name and the concept of
/// this component aren't final. I created this component to parallelize the `git am` and `kw
/// build` work so don't get too concerned with this boundary. I think we can fairly easilly merge
/// components if we understand they are highly coupled and are better together
pub struct Ktree {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
}

impl Ktree {
    pub fn new() -> Self {
        Self {
            command_tx: None,
            config: Config::default(),
        }
    }

    /// Here you will implement the logic of building and issuing the `git am` command based on the
    /// patch filepath provided. Of course, start small to see it working, then you can iterate and
    /// improve (check pre-conditions, add error-handling, change/add state for `draw()`, etc.)
    fn apply_patch(&self, patch_filepath: String) {
        todo!()
    }
}

impl Component for Ktree {
    /// You may or may not need to use this action transmitter
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    /// Same as the action transmitter, but I don't think initially you should need it
    fn register_config_handler(&mut self, config: Config) -> color_eyre::Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> color_eyre::Result<Option<Action>> {
        let action = match key.code {
            // Note: In reality, we will probably signal this action from the `tx` outside the
            // event handler (probably in another component); we don't want to have actions with
            // payloads in the event handler to be able to have all key events be defined in the
            // config file.
            KeyCode::Char('z') => Some(Action::KtreeApply("<patch-filepath>".to_string())),
            _ => None,
        };

        Ok(action)
    }

    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        match action {
            // Here the component will capture the action and dispatch the action handling to a
            // method.
            Action::KtreeApply(patch_filepath) => self.apply_patch(patch_filepath),
            _ => {}
        }
        Ok(None)
    }

    /// Initially, maybe an `Paragraph` reading from a `String` field of `Ktree` containing the
    /// output of a `git log -1 --format=%s` (commit message subject of HEAD) to test success/fail
    /// of `git am`.
    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        let _ = frame;
        let _ = area;
        Ok(())
    }
}
