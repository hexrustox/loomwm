use std::collections::hash_map::Entry;

use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor, delegate_shm,
    reexports::wayland_server::{
        Client,
        backend::ClientData,
        protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface},
    },
    wayland::{
        buffer::BufferHandler,
        compositor::{
            CompositorClientState, CompositorHandler, CompositorState, get_parent,
            is_sync_subsurface,
        },
        shm::{ShmHandler, ShmState},
    },
};

use crate::{state::WaylandState, utils::is_mapped, window::UnmappedWindowConfigurationState};

impl CompositorHandler for WaylandState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        if is_sync_subsurface(surface) {
            return;
        }

        let mut root_surface = surface.clone();
        while let Some(parent) = get_parent(&root_surface) {
            root_surface = parent;
        }

        if let Entry::Occupied(entry) = self.unmapped_windows.entry(root_surface) {
            if is_mapped(surface) {
                let unmapped = entry.remove();
                unmapped.inner.on_commit();
            } else {
                let unmapped = entry.get();
                let toplevel = unmapped.toplevel().clone();
                self.event_loop.insert_idle(move |state| {
                    if !toplevel.alive() {
                        return;
                    }

                    if let Some(unmapped) = state
                        .compositor
                        .unmapped_windows
                        .get_mut(toplevel.wl_surface())
                        && !unmapped.configured()
                    {
                        unmapped.state = UnmappedWindowConfigurationState::Configured;
                        // TEMP
                        toplevel.with_pending_state(|s| {
                           s.decoration_mode = Some(smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode::ClientSide) ;
                        });
                        toplevel.send_configure();
                    }
                });
            }
        } else if let Some(mapped) = self.find_mapped_window(surface) {
            mapped.inner.on_commit();

            // assume window surface will not be unmapped
        }

        // popup, layer shell & other surface
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {}

impl BufferHandler for WaylandState {
    fn buffer_destroyed(&mut self, buffer: &WlBuffer) {}
}

impl ShmHandler for WaylandState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_compositor!(WaylandState);
delegate_shm!(WaylandState);
