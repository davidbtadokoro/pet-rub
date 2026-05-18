use serde::{Deserialize, Serialize};
use strum::Display;

use crate::components::lei;

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    Suspend,
    Resume,
    Quit,
    ClearScreen,
    Error(String),
    Help,
    LeiSetMode(lei::LocalMode),
    LeiUpdateInbox,
    LeiEnterProcessing,
    LeiExitProcessing,
    LeiFetchPatchsets,
}
