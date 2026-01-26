use smithay::{
    delegate_xdg_shell,
    desktop::Window,
    input::{
        Seat,
        pointer::{Focus, GrabStartData as PointerGrabStartData},
    },
    reexports::wayland_server::{
        Resource,
        protocol::{wl_seat::WlSeat, wl_surface::WlSurface},
    },
    utils::Serial,
    wayland::shell::xdg::{
        PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    },
};

use crate::{input::move_grab::MoveGrab, state::WaylandState, window::MappedWindow};

impl XdgShellHandler for WaylandState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface);
        self.new_window(window);
    }

    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {}

    fn grab(
        &mut self,
        surface: smithay::wayland::shell::xdg::PopupSurface,
        seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat,
        serial: smithay::utils::Serial,
    ) {
    }

    fn reposition_request(
        &mut self,
        surface: smithay::wayland::shell::xdg::PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: WlSeat, serial: Serial) {
        let seat = Seat::from_resource(&seat).unwrap();

        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let pointer = seat.get_pointer().unwrap();

            let window = self.mapped_window_lookup(wl_surface);
            if let Some(MappedWindow {
                inner, location, ..
            }) = window
            {
                let grab = MoveGrab::new(start_data, inner.clone(), location.to_f64());
                pointer.set_grab(self, grab, serial, Focus::Clear);
            }
        }
    }
}

delegate_xdg_shell!(WaylandState);

fn check_grab(
    seat: &Seat<WaylandState>,
    surface: &WlSurface,
    serial: Serial,
) -> Option<PointerGrabStartData<WaylandState>> {
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
