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
            is_sync_subsurface, with_states,
        },
        shell::xdg::XdgToplevelSurfaceData,
        shm::{ShmHandler, ShmState},
    },
};

use crate::{
    input::resize_grab,
    state::WaylandState,
    utils::is_mapped,
    window::{UnmappedWindowConfigurationState, rule::WindowRuleMatch},
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
                let window = unmapped.inner;
                let UnmappedWindowConfigurationState::Configured {
                    focus,
                    floating,
                    location,
                    workspace,
                } = unmapped.state
                else {
                    unreachable!()
                };

                window.on_commit();
                self.add_window(window, focus, floating, workspace);
            } else {
                let unmapped = entry.get();

                let (app_id, title) = with_states(surface, |surface_data| {
                    if let Some(attrs) = surface_data.data_map.get::<XdgToplevelSurfaceData>() {
                        let attrs = attrs.lock().unwrap();
                        (attrs.app_id.clone(), attrs.title.clone())
                    } else {
                        (None, None)
                    }
                });

                let candidate = WindowRuleMatch {
                    app_id,
                    title,
                    focus: None,
                    float: None,
                    workspace: None,
                };
                let properties = self.window_rules.get_config(&candidate);

                let config_state = UnmappedWindowConfigurationState::Configured {
                    focus: properties.focus,
                    floating: properties.float.is_some(),
                    location: properties.float.and_then(|f| f.location),
                    workspace: properties.workspace,
                };

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
                        unmapped.state = config_state;
                        toplevel.with_pending_state(|state| {
                            state.decoration_mode = Some(properties.decoration.into());
                        });
                        toplevel.send_configure();
                    }
                });
            }
        } else if let Some(mapped) = self.find_mapped_window_mut(surface) {
            mapped.inner.on_commit();
            resize_grab::handle_commit(mapped);

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
