use smithay::{
    delegate_data_device, delegate_output, delegate_seat,
    input::{Seat, SeatHandler, SeatState},
    reexports::wayland_server::{Resource, protocol::wl_surface::WlSurface},
    wayland::{
        output::OutputHandler,
        selection::{
            SelectionHandler,
            data_device::{
                ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
                set_data_device_focus,
            },
        },
    },
};

use crate::state::WMState;

mod compositor;
mod xdg_shell;

pub use compositor::ClientState;

impl SeatHandler for WMState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client.clone());
    }
}

delegate_seat!(WMState);

impl SelectionHandler for WMState {
    type SelectionUserData = ();
}

impl ClientDndGrabHandler for WMState {}
impl ServerDndGrabHandler for WMState {}

impl DataDeviceHandler for WMState {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

delegate_data_device!(WMState);

impl OutputHandler for WMState {}
delegate_output!(WMState);
