use std::{collections::HashMap, ffi::OsString, rc::Rc, sync::Arc};

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
    config::{Config, GeneralConfig, KeyConfig, PointerConfig},
    handlers::ClientState,
    input::KeyModifiers,
    monitor::{LayoutSet, Monitors},
    window::{UnmappedWindow, rule::WindowRules},
};

pub struct WaylandState {
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

    pub key_modifiers: KeyModifiers,

    pub general_config: GeneralConfig,
    pub window_rules: WindowRules,
    pub layout_set: Rc<LayoutSet>,
    pub default_layout: Rc<str>,
    pub key_config: KeyConfig,
    pub pointer_config: PointerConfig,
}

impl WaylandState {
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
        seat.add_keyboard(Default::default(), 200, 25).unwrap();
        seat.add_pointer();

        let xdg_decoration_state = XdgDecorationState::new::<Self>(&dh);

        let socket_name = {
            {
                let listening_socket = ListeningSocketSource::new_auto().unwrap();
                let socket_name = listening_socket.socket_name().to_os_string();

                event_loop
                    .insert_source(listening_socket, move |client_stream, _, state| {
                        state
                            .compositor
                            .display_handle
                            .insert_client(client_stream, Arc::new(ClientState::default()))
                            .unwrap();
                    })
                    .expect("Failed to init the wayland event source.");

                event_loop
                    .insert_source(
                        Generic::new(display, Interest::READ, Mode::Level),
                        |_, display, state| {
                            unsafe {
                                display
                                    .get_mut()
                                    .dispatch_clients(&mut state.compositor)
                                    .unwrap();
                            }
                            Ok(PostAction::Continue)
                        },
                    )
                    .unwrap();

                socket_name
            }
        };

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

            key_modifiers: KeyModifiers::empty(),

            general_config: config.general,
            window_rules: config.window_rules,
            layout_set: Rc::new(config.layout.layouts),
            default_layout: Rc::from(config.layout.default),
            key_config: config.key,
            pointer_config: config.pointer,
        }
    }

    pub fn update_config(&mut self, config: Config) {
        self.general_config = config.general;
        self.window_rules = config.window_rules;
        self.layout_set = Rc::new(config.layout.layouts);
        self.default_layout = Rc::from(config.layout.default);
        self.key_config = config.key;
        self.pointer_config = config.pointer;

        self.apply_rule_to_mapped_windows();
    }
}
