use std::{cell::RefCell, collections::HashMap, ffi::OsString, rc::Rc, sync::Arc, time::Instant};

use smithay::{
    desktop::{PopupManager, Space, Window},
    input::{Seat, SeatState},
    reexports::{
        calloop::{Interest, LoopHandle, LoopSignal, Mode, PostAction, generic::Generic},
        wayland_server::{Display, DisplayHandle, protocol::wl_surface::WlSurface},
    },
    wayland::{
        compositor::CompositorState,
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::xdg::{XdgShellState, decoration::XdgDecorationState},
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};

use crate::{
    CompositorData,
    config::{AssistantConfig, Config, GeneralConfig, KeyConfig, PointerConfig},
    handlers::ClientState,
    input::{KeyAction, KeyModifiers},
    monitor::{BackendDevice, LayoutHistory, LayoutSet, Monitors},
    window::{UnmappedWindow, rule::WindowRules},
};

pub struct WindowManagerState {
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,

    pub event_loop: LoopHandle<'static, CompositorData>,
    pub event_signal: LoopSignal,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Self>,
    pub data_device_state: DataDeviceState,
    pub popups: PopupManager,
    pub seat: Seat<Self>,

    pub xdg_decoration_state: XdgDecorationState,

    pub space: Space<Window>,
    pub unmapped_windows: HashMap<WlSurface, UnmappedWindow>,
    pub monitors: Monitors,

    pub general_config: GeneralConfig,
    pub window_rules: WindowRules,
    pub layout_set: Rc<RefCell<LayoutSet>>,
    pub default_layout: String,
    pub key_config: KeyConfig,
    pub pointer_config: PointerConfig,

    pub key_modifiers: KeyModifiers,
    pub repeat_action: Option<KeyAction>,

    pub assistant_config: AssistantConfig,
    pub backend_device: BackendDevice,
    pub save_at: Option<Instant>,
    pub layout_history: LayoutHistory,
}

impl WindowManagerState {
    pub fn new(
        event_loop: LoopHandle<'static, CompositorData>,
        event_signal: LoopSignal,
        display: Display<Self>,
        config: Config,
    ) -> Self {
        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let popups = PopupManager::default();

        let mut seat = seat_state.new_wl_seat(&dh, "winit");
        seat.add_keyboard(
            Default::default(),
            config.key.repeat_delay as i32,
            config.key.repeat_rate as i32,
        )
        .expect("Failed to create keyboard for seat");
        seat.add_pointer();

        let xdg_decoration_state = XdgDecorationState::new::<Self>(&dh);

        let socket_name = {
            {
                let listening_socket =
                    ListeningSocketSource::new_auto().expect("Failed to bind socket");
                let socket_name = listening_socket.socket_name().to_os_string();

                event_loop
                    .insert_source(listening_socket, move |client_stream, _, state| {
                        state
                            .compositor
                            .display_handle
                            .insert_client(client_stream, Arc::new(ClientState::default()))
                            .expect("Failed to insert client");
                    })
                    .expect("Failed to init the wayland event source");

                event_loop
                    .insert_source(
                        Generic::new(display, Interest::READ, Mode::Level),
                        |_, display, state| {
                            unsafe {
                                display
                                    .get_mut()
                                    .dispatch_clients(&mut state.compositor)
                                    .expect("Failed to dispatch client requests");
                            }
                            Ok(PostAction::Continue)
                        },
                    )
                    .expect("Failed to init display event source");

                socket_name
            }
        };

        if config.assistant.enable {
            event_loop.insert_idle(|data| {
                data.compositor.backend_device.init();
            });
        }

        Self {
            socket_name,
            display_handle: dh,

            event_loop,
            event_signal,

            compositor_state,
            xdg_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            popups,
            seat,

            xdg_decoration_state,

            space: Space::default(),
            unmapped_windows: HashMap::new(),
            monitors: Monitors::default(),

            general_config: config.general,
            window_rules: config.window_rules,
            layout_set: Rc::new(RefCell::new(config.layouts.layout_set)),
            default_layout: config.layouts.default,
            key_config: config.key,
            pointer_config: config.pointer,

            key_modifiers: KeyModifiers::empty(),
            repeat_action: None,

            assistant_config: config.assistant,
            backend_device: BackendDevice::new(),
            save_at: None,
            layout_history: LayoutHistory::new(),
        }
    }

    pub fn refresh(&mut self) {
        self.popups.cleanup();

        self.recompute_window_rules();

        self.refresh_windows();

        self.space.refresh();
    }
}
