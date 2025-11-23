use std::{collections::HashMap, ffi::OsString, sync::Arc};

use anyhow::anyhow;
use smithay::{
    desktop::{PopupManager, Space, Window, WindowSurfaceType},
    input::{Seat, SeatState},
    output::Output,
    reexports::{
        calloop::{
            EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction, generic::Generic,
        },
        wayland_server::{
            Display, DisplayHandle,
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
        },
    },
    utils::{Logical, Point},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::xdg::XdgShellState,
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};

use crate::{CallLoopData, layout::Layout, window::unmapped::UnmappedWindow};

pub struct Smallvil {
    pub start_time: std::time::Instant,
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,

    pub space: Space<Window>,
    pub loop_signal: LoopSignal,
    pub event_loop: LoopHandle<'static, CallLoopData>,

    // Smithay State
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Smallvil>,
    pub data_device_state: DataDeviceState,
    pub popups: PopupManager,
    pub seat: Seat<Self>,

    pub unmapped_windows: HashMap<WlSurface, UnmappedWindow>,
    pub layout: Layout,
}

impl Smallvil {
    pub fn new(
        event_loop: LoopHandle<'static, CallLoopData>,
        loop_signal: LoopSignal,
        display: Display<Self>,
    ) -> Self {
        let start_time = std::time::Instant::now();

        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let popups = PopupManager::default();

        // A seat is a group of keyboards, pointer and touch devices.
        // A seat typically has a pointer and maintains a keyboard focus and a pointer focus.
        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "winit");

        // Notify clients that we have a keyboard, for the sake of the example we assume that keyboard is always present.
        // You may want to track keyboard hot-plug in real compositor.
        seat.add_keyboard(Default::default(), 200, 25).unwrap();

        // Notify clients that we have a pointer (mouse)
        // Here we assume that there is always pointer plugged in
        seat.add_pointer();

        // A space represents a two-dimensional plane. Windows and Outputs can be mapped onto it.
        //
        // Windows get a position and stacking order through mapping.
        // Outputs become views of a part of the Space and can be rendered via Space::render_output.
        let space = Space::default();

        let socket_name = {
            // Creates a new listening socket, automatically choosing the next available `wayland` socket name.
            let listening_socket = ListeningSocketSource::new_auto().unwrap();

            // Get the name of the listening socket.
            // Clients will connect to this socket.
            let socket_name = listening_socket.socket_name().to_os_string();

            event_loop
                .insert_source(listening_socket, move |client_stream, _, data| {
                    // Inside the callback, you should insert the client into the display.
                    //
                    // You may also associate some data with the client when inserting the client.
                    data.state
                        .display_handle
                        .insert_client(client_stream, Arc::new(ClientState::default()))
                        .unwrap();
                })
                .expect("Failed to init the wayland event source.");

            // You also need to add the display itself to the event loop, so that client events will be processed by wayland-server.
            event_loop
                .insert_source(
                    Generic::new(display, Interest::READ, Mode::Level),
                    |_, display, data| {
                        // Safety: we don't drop the display
                        unsafe {
                            display.get_mut().dispatch_clients(&mut data.state).unwrap();
                        }
                        Ok(PostAction::Continue)
                    },
                )
                .unwrap();

            socket_name
        };

        Self {
            start_time,
            socket_name,
            display_handle: dh,

            space,
            loop_signal,
            event_loop,

            compositor_state,
            xdg_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            popups,
            seat,

            unmapped_windows: HashMap::new(),
            layout: Layout::new(),
        }
    }

    // pub fn surface_under(
    //     &self,
    //     pos: Point<f64, Logical>,
    // ) -> Option<(WlSurface, Point<f64, Logical>)> {
    //     self.windows
    //         .element_under(pos)
    //         .and_then(|(window, location)| {
    //             window
    //                 .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
    //                 .map(|(s, p)| (s, (p + location).to_f64()))
    //         })
    // }

    pub fn add_output(&mut self, output: Output) {
        let _global = output.create_global::<Smallvil>(&self.display_handle);
        self.space.map_output(&output, (0, 0));
        self.layout.add_output(output);
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}
