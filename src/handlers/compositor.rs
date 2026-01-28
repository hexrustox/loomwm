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

use crate::{
    state::WaylandState,
    utils::is_mapped,
    window::{MappedWindow, UnmappedWindowConfigurationState},
};

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
                self.workspaces
                    .get_active()
                    .new_floating_window(MappedWindow::new(unmapped.inner));
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
                        toplevel.send_configure();
                    }
                });
            }
        } else if let Some(mapped) = self.mapped_window_lookup(surface) {
            mapped.inner.on_commit();
        }
        // assume window surface will not be unmapped
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
