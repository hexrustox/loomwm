use notify::{Event, EventKind, INotifyWatcher, Watcher, event::ModifyKind};
use smithay::reexports::calloop::{EventLoop, channel};
use tracing::error;

use crate::{CompositorData, config::Config, state::WindowManagerState};

pub fn init(event_loop: &EventLoop<CompositorData>) -> Option<INotifyWatcher> {
    let (sender, reciever) = channel::channel();

    let Ok(mut watcher) = notify::recommended_watcher(move |event| {
        if let Ok(Event {
            kind: EventKind::Modify(ModifyKind::Data(_)),
            ..
        }) = event
        {
            sender.send(()).unwrap()
        }
    }) else {
        error!("Failed to init config watcher");
        return None;
    };

    if let Some(parent) = Config::path().parent() {
        if let Err(e) = watcher.watch(parent, notify::RecursiveMode::NonRecursive) {
            error!("Failed to watch path {parent:?}: {e}");
            return None;
        }
    } else {
        error!("Config file cannot be at root");
    }

    event_loop
        .handle()
        .insert_source(reciever, move |event, _, data| {
            if let channel::Event::Msg(_) = event {
                match Config::read() {
                    Ok(config) => {
                        data.compositor.update_config(config);
                    }
                    Err(e) => {
                        error!("{e}");
                    }
                }
            }
        })
        .expect("Failed to init watcher event source");

    Some(watcher)
}

impl WindowManagerState {
    fn update_config(&mut self, config: Config) {
        // REMIND
        #[cfg(test)]
        let Config {
            general: _,
            window_rules: _,
            layouts: _,
            key: _,
            pointer: _,
            assistant: _,
        } = config;

        if self.window_rules.get_hash() != config.window_rules.get_hash() {
            self.window_rules = config.window_rules;
            self.apply_rule_to_mapped_windows();
        }

        self.general_config = config.general;

        // TODO live reload
        // self.layout_set = Rc::new(config.layouts.layout_set);
        // self.default_layout = Rc::from(config.layouts.default);

        self.get_keyboard().change_repeat_info(
            config.key.repeat_rate as i32,
            config.key.repeat_delay as i32,
        );
        self.key_config = config.key;

        self.pointer_config = config.pointer;

        if config.assistant.enable {
            self.event_loop.insert_idle(|data| {
                data.compositor.backend_device.init();
            });
        }
        self.assistant_config = config.assistant;
    }
}
