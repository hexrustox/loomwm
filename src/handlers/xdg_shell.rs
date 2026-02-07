use smithay::{
    delegate_xdg_shell,
    desktop::Window,
    input::{
        Seat,
        pointer::{Focus, GrabStartData as PointerGrabStartData},
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            Resource,
            protocol::{wl_seat::WlSeat, wl_surface::WlSurface},
        },
    },
    utils::{Rectangle, Serial},
    wayland::shell::xdg::{
        PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    },
};

use crate::{
    input::{move_grab::MoveGrab, resize_grab::ResizeGrab},
    state::WaylandState,
    window::{MappedWindow, UnmappedWindow},
};

impl XdgShellHandler for WaylandState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, toplevel: ToplevelSurface) {
        let window = Window::new_wayland_window(toplevel);
        let surface = window.toplevel().unwrap().wl_surface().clone();
        self.unmapped_windows
            .insert(surface, UnmappedWindow::new(window));
    }

    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {}

    fn grab(
        &mut self,
        _surface: smithay::wayland::shell::xdg::PopupSurface,
        _seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat,
        _serial: smithay::utils::Serial,
    ) {
    }

    fn reposition_request(
        &mut self,
        _surface: smithay::wayland::shell::xdg::PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: WlSeat, serial: Serial) {
        let seat = Seat::from_resource(&seat).unwrap();

        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let pointer = seat.get_pointer().unwrap();

            let window = self.find_mapped_window(wl_surface);
            if let Some((
                MappedWindow {
                    window: inner,
                    location,
                    ..
                },
                ..,
            )) = window
            {
                let grab = MoveGrab::new(start_data, inner.clone(), location.to_f64());
                pointer.set_grab(self, grab, serial, Focus::Clear);
            }
        }
    }

    fn resize_request(
        &mut self,
        toplevel: ToplevelSurface,
        seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat,
        serial: Serial,
        edges: smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge,
    ) {
        let seat = Seat::from_resource(&seat).unwrap();

        let surface = toplevel.wl_surface();

        if let Some(start_data) = check_grab(&seat, surface, serial) {
            let pointer = seat.get_pointer().unwrap();

            let (mapped, ..) = self.find_mapped_window(surface).unwrap();
            let initial_window_location = mapped.location;
            let initial_window_size = mapped.window.geometry().size;

            toplevel.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
            });

            toplevel.send_pending_configure();

            let grab = ResizeGrab::new(
                start_data,
                mapped.window.clone(),
                edges.into(),
                Rectangle::new(initial_window_location, initial_window_size),
            );

            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn toplevel_destroyed(&mut self, toplevel: ToplevelSurface) {
        self.remove_mapped_window(toplevel.wl_surface());
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
