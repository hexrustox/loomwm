use std::collections::hash_map::Entry;

use crate::{
    Smallvil,
    grabs::resize_grab,
    state::ClientState,
    window::{mapped::MappedWindow, unmapped::UnmappedWindow},
};
use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor, delegate_shm,
    reexports::wayland_server::{
        Client,
        protocol::{wl_buffer, wl_surface::WlSurface},
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

use super::xdg_shell;

impl CompositorHandler for Smallvil {
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

        if *surface == root_surface {
            //
            if let Entry::Occupied(entry) = self.unmapped_windows.entry(surface.clone()) {
                if entry.get().is_mapped() {
                    let UnmappedWindow { window, state } = entry.remove();
                    window.on_commit();

                    let mapped = MappedWindow::new(window);
                    let toplevel = mapped.toplevel().clone();
                    self.layout.add_window(mapped);
                    self.focus_window(&toplevel);
                } else {
                    let window = entry.get();
                    if !window.is_configured() {
                        let toplevel = window.toplevel().clone();
                        self.queue_initial_configure(toplevel);
                    }
                }
            }
        }
    }
}

impl BufferHandler for Smallvil {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for Smallvil {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_compositor!(Smallvil);
delegate_shm!(Smallvil);
