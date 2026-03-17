use notify::INotifyWatcher;

use crate::{backend::Backend, state::WindowManagerState};

pub mod backend;
pub mod config;
pub mod handlers;
pub mod input;
pub mod monitor;
pub mod path;
pub mod state;
pub mod utils;
pub mod watcher;
pub mod window;

pub struct CompositorData {
    pub compositor: WindowManagerState,
    pub backend: Backend,
    pub watcher: Option<INotifyWatcher>,
}
