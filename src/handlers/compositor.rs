use smithay::{
    delegate_compositor, delegate_shm,
    reexports::wayland_server::{
        Client,
        backend::ClientData,
        protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface},
    },
    wayland::{
        buffer::BufferHandler,
        compositor::{CompositorClientState, CompositorHandler, CompositorState},
        shm::{ShmHandler, ShmState},
    },
};

use crate::state::WMState;

impl CompositorHandler for WMState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {}
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {}

impl BufferHandler for WMState {
    fn buffer_destroyed(&mut self, buffer: &WlBuffer) {}
}

impl ShmHandler for WMState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_compositor!(WMState);
delegate_shm!(WMState);
