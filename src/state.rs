use std::{collections::HashMap, ffi::OsString, sync::Arc};

use smithay::{
    desktop::{PopupManager, Space, Window},
    input::{Seat, SeatState},
    reexports::{
        calloop::{Interest, LoopHandle, LoopSignal, Mode, PostAction, generic::Generic},
        wayland_server::{Display, DisplayHandle, protocol::wl_surface::WlSurface},
    },
    wayland::{
        compositor::CompositorState, output::OutputManagerState,
        selection::data_device::DataDeviceState, shell::xdg::XdgShellState, shm::ShmState,
        socket::ListeningSocketSource,
    },
};

use crate::{AppState, handlers::ClientState, window::UnmappedWindow, workspace::Workspaces};

pub struct WaylandState {
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,

    pub event_loop: LoopHandle<'static, AppState>,
    pub event_signal: LoopSignal,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Self>,
    pub data_device_state: DataDeviceState,
    pub popups: PopupManager,
    pub seat: Seat<Self>,

    pub space: Space<Window>,
    pub unmapped_windows: HashMap<WlSurface, UnmappedWindow>,
    pub workspaces: Workspaces,
}

impl WaylandState {
    pub fn new(
        event_loop: LoopHandle<'static, AppState>,
        event_signal: LoopSignal,
        display: Display<Self>,
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

            space: Space::default(),
            unmapped_windows: HashMap::new(),
            workspaces: Workspaces::default(),
        }
    }
}
