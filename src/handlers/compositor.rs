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
    monitor::FoundMappedWindow,
    state::WindowManagerState,
    utils::is_mapped,
    window::{UnmappedWindowState, rule::WindowProperties},
};

impl CompositorHandler for WindowManagerState {
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

        if surface == &root_surface {
            if let Entry::Occupied(entry) = self.unmapped_windows.entry(surface.clone()) {
                if is_mapped(surface) {
                    let unmapped = entry.remove();
                    let window = unmapped.window;
                    let properties = match unmapped.state {
                        UnmappedWindowState::Configured(x) => x,
                        UnmappedWindowState::NotConfigured => WindowProperties::default(),
                    };

                    window.on_commit();
                    self.register_new_window(window, properties);
                } else {
                    let unmapped = entry.get();

                    let workspace_name = self
                        .monitors
                        .get_monitor()
                        .get_active_workspace_name()
                        .clone();
                    let properties = self.window_rules.get_opening_properties(
                        unmapped.toplevel().wl_surface(),
                        workspace_name,
                        None,
                    );

                    let config_state = UnmappedWindowState::Configured(properties);

                    let toplevel = unmapped.toplevel().clone();
                    self.event_loop.insert_idle(move |data| {
                        if !toplevel.alive() {
                            return;
                        }

                        if let Some(unmapped) = data
                            .compositor
                            .unmapped_windows
                            .get_mut(toplevel.wl_surface())
                            && !unmapped.configured()
                        {
                            unmapped.state = config_state;
                            toplevel.send_configure();
                        }
                    });
                }

                return;
            }

            // previously-mapped root
            if let Some(FoundMappedWindow { mut mapped, .. }) =
                self.find_mapped_window_by_surface(surface)
            {
                mapped.window().on_commit();
                mapped.resize_handle_commit();

                // TODO handle toplevel unmapped
                return;
            }
        }

        // non-root
        if let Some(FoundMappedWindow { mapped, .. }) =
            self.find_mapped_window_by_surface(&root_surface)
        {
            mapped.window().on_commit();

            return;
        }

        // popup
        self.popup_handle_commit(surface);

        // layer shell & other surface
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {}

impl BufferHandler for WindowManagerState {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl ShmHandler for WindowManagerState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_compositor!(WindowManagerState);
delegate_shm!(WindowManagerState);
