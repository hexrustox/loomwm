use smithay::{
    delegate_xdg_shell,
    desktop::{PopupKind, Window, find_popup_root_surface, get_popup_toplevel_coords},
    input::{
        Seat,
        pointer::{Focus, GrabStartData as PointerGrabStartData},
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            Resource,
            protocol::{wl_output::WlOutput, wl_seat::WlSeat, wl_surface::WlSurface},
        },
    },
    utils::{Rectangle, Serial},
    wayland::shell::xdg::{
        PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    },
};
use tracing::warn;

use crate::{
    input::grabs::{floating_resize_grab::FloatingResizeGrab, move_grab::MoveGrab},
    monitor::{FoundMappedWindow, TileTreeWindow},
    state::WindowManagerState,
    window::{UnmappedWindow, WindowRole},
};

impl XdgShellHandler for WindowManagerState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, toplevel: ToplevelSurface) {
        let window = Window::new_wayland_window(toplevel);
        let surface = window
            .toplevel()
            .expect("No X11 support")
            .wl_surface()
            .clone();

        assert_eq!(
            self.unmapped_windows
                .insert(surface, UnmappedWindow::new(window)),
            None,
        );
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn grab(
        &mut self,
        _surface: smithay::wayland::shell::xdg::PopupSurface,
        _seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat,
        _serial: smithay::utils::Serial,
    ) {
    }

    fn reposition_request(
        &mut self,
        surface: smithay::wayland::shell::xdg::PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        surface.with_pending_state(|state| {
            let geometry = positioner.get_geometry();
            state.geometry = geometry;
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    fn move_request(&mut self, toplevel: ToplevelSurface, seat: WlSeat, serial: Serial) {
        if !self.general_config.allow_move_request {
            warn!("Client move request ignored");
            return;
        }

        let Some(seat) = Seat::from_resource(&seat) else {
            return;
        };

        let surface = toplevel.wl_surface();

        if let Some(start_data) = check_grab(&seat, surface, serial)
            && let Some(pointer) = seat.get_pointer()
            && let Some(FoundMappedWindow { mapped, .. }) = self.find_mapped_window(surface)
            && mapped.is_floating()
        {
            let grab = MoveGrab::new(start_data, mapped.clone(), mapped.get_location().to_f64());
            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn resize_request(
        &mut self,
        toplevel: ToplevelSurface,
        seat: WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        if !self.general_config.allow_resize_request {
            warn!("Client resize request ignored");
            return;
        }

        let Some(seat) = Seat::from_resource(&seat) else {
            return;
        };

        let surface = toplevel.wl_surface();

        if let Some(start_data) = check_grab(&seat, surface, serial)
            && let Some(pointer) = seat.get_pointer()
            && let Some(FoundMappedWindow { mapped, .. }) = self.find_mapped_window(surface)
            && mapped.is_floating()
        {
            let grab = FloatingResizeGrab::new(
                start_data,
                mapped.clone(),
                edges.into(),
                Rectangle::new(mapped.get_location(), mapped.get_geometry_size()),
            );
            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn toplevel_destroyed(&mut self, toplevel: ToplevelSurface) {
        if let Some(FoundMappedWindow { workspace_name, .. }) =
            self.remove_mapped_window(toplevel.wl_surface())
        {
            self.restore_workspace_focus(&workspace_name);
        };
    }

    // TODO minimize, maximize, fullscreen
    fn maximize_request(&mut self, toplevel: ToplevelSurface) {
        if !self.general_config.allow_resize_request {
            warn!("Client maximize request ignored");
            return;
        }
        self.toggle_window_occupant(toplevel.wl_surface(), WindowRole::Maximized, Some(true));
    }

    fn unmaximize_request(&mut self, toplevel: ToplevelSurface) {
        self.toggle_window_occupant(toplevel.wl_surface(), WindowRole::Maximized, Some(false));
    }

    fn fullscreen_request(&mut self, toplevel: ToplevelSurface, _output: Option<WlOutput>) {
        if !self.general_config.allow_resize_request {
            warn!("Client fullscreen request ignored");
            return;
        }
        self.toggle_window_occupant(toplevel.wl_surface(), WindowRole::Fullscreen, Some(true));
    }

    fn unfullscreen_request(&mut self, toplevel: ToplevelSurface) {
        self.toggle_window_occupant(toplevel.wl_surface(), WindowRole::Fullscreen, Some(false));
    }

    // TODO
    // fn app_id_changed(&mut self, surface: ToplevelSurface) {
    // }
    // fn title_changed(&mut self, surface: ToplevelSurface) {
    // }
}

delegate_xdg_shell!(WindowManagerState);

fn check_grab(
    seat: &Seat<WindowManagerState>,
    surface: &WlSurface,
    serial: Serial,
) -> Option<PointerGrabStartData<WindowManagerState>> {
    let pointer = seat.get_pointer()?;

    if !pointer.has_grab(serial) {
        return None;
    }

    let start_data = pointer.grab_start_data()?;

    let (focus, _) = start_data.focus.as_ref()?;
    if !focus.id().same_client_as(&surface.id()) {
        return None;
    }

    Some(start_data)
}

impl WindowManagerState {
    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let Some(FoundMappedWindow { mapped, .. }) = self.find_mapped_window(&root) else {
            return;
        };

        let output = self.space.outputs().next().unwrap();
        let output_geo = self.space.output_geometry(output).unwrap();
        let window_geo = mapped.window().geometry();

        // The target geometry for the positioner should be relative to its parent's geometry, so
        // we will compute that here.
        let mut target = output_geo;
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= window_geo.loc;

        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }

    pub fn popup_handle_commit(&mut self, surface: &WlSurface) {
        self.popups.commit(surface);
        if let Some(kind) = self.popups.find_popup(surface) {
            match kind {
                PopupKind::Xdg(ref xdg) => {
                    if !xdg.is_initial_configure_sent() {
                        xdg.send_configure()
                            .expect("Popup initial configure failed");
                    }
                }
                PopupKind::InputMethod(_) => {}
            }
        }
    }
}
