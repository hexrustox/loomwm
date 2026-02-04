use smithay::{
    desktop::Window,
    input::pointer::{
        AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
        GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent,
        GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData as PointerGrabStartData,
        MotionEvent, PointerGrab, PointerInnerHandle, RelativeMotionEvent,
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
};

use crate::state::WaylandState;

pub struct MoveGrab {
    start_data: PointerGrabStartData<WaylandState>,
    window: Window,
    last_location: Point<f64, Logical>,
}

impl MoveGrab {
    pub fn new<T: Into<Point<f64, Logical>>>(
        start_data: PointerGrabStartData<WaylandState>,
        window: Window,
        last_location: T,
    ) -> Self {
        Self {
            start_data,
            window,
            last_location: last_location.into(),
        }
    }
}

impl PointerGrab<WaylandState> for MoveGrab {
    fn motion(
        &mut self,
        data: &mut WaylandState,
        handle: &mut PointerInnerHandle<'_, WaylandState>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        // While the grab is active, no client has pointer focus
        handle.motion(data, None, event);

        let delta = event.location - self.start_data.location;
        let new_location = self.last_location + delta;
        if let Some(mapped) =
            data.find_mapped_window_mut(self.window.toplevel().unwrap().wl_surface())
        {
            mapped.location = new_location.to_i32_round();
        }
    }

    fn relative_motion(
        &mut self,
        data: &mut WaylandState,
        handle: &mut PointerInnerHandle<'_, WaylandState>,
        focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(
        &mut self,
        data: &mut WaylandState,
        handle: &mut PointerInnerHandle<'_, WaylandState>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);

        if !handle.current_pressed().contains(&self.start_data.button) {
            // No more buttons are pressed, release the grab.
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn axis(
        &mut self,
        data: &mut WaylandState,
        handle: &mut PointerInnerHandle<'_, WaylandState>,
        details: AxisFrame,
    ) {
        handle.axis(data, details)
    }

    fn frame(
        &mut self,
        data: &mut WaylandState,
        handle: &mut PointerInnerHandle<'_, WaylandState>,
    ) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut WaylandState,
        handle: &mut PointerInnerHandle<'_, WaylandState>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event)
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut WaylandState,
        handle: &mut PointerInnerHandle<'_, WaylandState>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event)
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut WaylandState,
        handle: &mut PointerInnerHandle<'_, WaylandState>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event)
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut WaylandState,
        handle: &mut PointerInnerHandle<'_, WaylandState>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event)
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut WaylandState,
        handle: &mut PointerInnerHandle<'_, WaylandState>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event)
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut WaylandState,
        handle: &mut PointerInnerHandle<'_, WaylandState>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event)
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut WaylandState,
        handle: &mut PointerInnerHandle<'_, WaylandState>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event)
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut WaylandState,
        handle: &mut PointerInnerHandle<'_, WaylandState>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event)
    }

    fn start_data(&self) -> &PointerGrabStartData<WaylandState> {
        &self.start_data
    }

    fn unset(&mut self, _data: &mut WaylandState) {}
}
