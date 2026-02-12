use crate::{
    input::{WindowDirection, grabs::floating_resize_grab::ResizeEdge},
    state::WindowManagerState,
};
use smithay::{
    input::{
        SeatHandler,
        pointer::{
            AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent,
            GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent,
            GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent,
            GrabStartData as PointerGrabStartData, MotionEvent, PointerGrab, PointerInnerHandle,
            RelativeMotionEvent,
        },
    },
    utils::{Logical, Rectangle, Size},
};

pub struct TilingResizeGrab {
    start_data: PointerGrabStartData<WindowManagerState>,
    edges: ResizeEdge,
    initial_rect: Rectangle<i32, Logical>,
    last_size: Size<i32, Logical>,
}

impl TilingResizeGrab {
    pub fn new(
        start_data: PointerGrabStartData<WindowManagerState>,
        edges: ResizeEdge,
        initial_rect: Rectangle<i32, Logical>,
    ) -> Self {
        Self {
            start_data,
            edges,
            initial_rect,
            last_size: initial_rect.size,
        }
    }
}

impl PointerGrab<WindowManagerState> for TilingResizeGrab {
    fn motion(
        &mut self,
        data: &mut WindowManagerState,
        handle: &mut PointerInnerHandle<'_, WindowManagerState>,
        _focus: Option<(
            <WindowManagerState as SeatHandler>::PointerFocus,
            smithay::utils::Point<f64, smithay::utils::Logical>,
        )>,
        event: &MotionEvent,
    ) {
        handle.motion(data, None, event);

        let delta = event.location - self.start_data.location;
        let initial = self.initial_rect.size;

        let mut update_dim = |edge: ResizeEdge,
                              direction: WindowDirection,
                              initial_size: i32,
                              delta_val: f64,
                              last_size: &mut i32| {
            if self.edges.intersects(edge) {
                let new_size = (initial_size as f64 + delta_val) as i32;

                if new_size != *last_size {
                    data.resize_focused_tiling_window_in_edge(direction, new_size);
                    *last_size = new_size;
                }
            }
        };

        update_dim(
            ResizeEdge::LEFT,
            WindowDirection::Left,
            initial.w,
            -delta.x,
            &mut self.last_size.w,
        );
        update_dim(
            ResizeEdge::RIGHT,
            WindowDirection::Right,
            initial.w,
            delta.x,
            &mut self.last_size.w,
        );

        update_dim(
            ResizeEdge::TOP,
            WindowDirection::Up,
            initial.h,
            -delta.y,
            &mut self.last_size.h,
        );
        update_dim(
            ResizeEdge::BOTTOM,
            WindowDirection::Down,
            initial.h,
            delta.y,
            &mut self.last_size.h,
        );
    }

    fn relative_motion(
        &mut self,
        data: &mut WindowManagerState,
        handle: &mut PointerInnerHandle<'_, WindowManagerState>,
        focus: Option<(
            <WindowManagerState as SeatHandler>::PointerFocus,
            smithay::utils::Point<f64, smithay::utils::Logical>,
        )>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(
        &mut self,
        data: &mut WindowManagerState,
        handle: &mut PointerInnerHandle<'_, WindowManagerState>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);

        if !handle.current_pressed().contains(&self.start_data.button) {
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn axis(
        &mut self,
        data: &mut WindowManagerState,
        handle: &mut PointerInnerHandle<'_, WindowManagerState>,
        details: AxisFrame,
    ) {
        handle.axis(data, details)
    }

    fn frame(
        &mut self,
        data: &mut WindowManagerState,
        handle: &mut PointerInnerHandle<'_, WindowManagerState>,
    ) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut WindowManagerState,
        handle: &mut PointerInnerHandle<'_, WindowManagerState>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event)
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut WindowManagerState,
        handle: &mut PointerInnerHandle<'_, WindowManagerState>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event)
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut WindowManagerState,
        handle: &mut PointerInnerHandle<'_, WindowManagerState>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event)
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut WindowManagerState,
        handle: &mut PointerInnerHandle<'_, WindowManagerState>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event)
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut WindowManagerState,
        handle: &mut PointerInnerHandle<'_, WindowManagerState>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event)
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut WindowManagerState,
        handle: &mut PointerInnerHandle<'_, WindowManagerState>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event)
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut WindowManagerState,
        handle: &mut PointerInnerHandle<'_, WindowManagerState>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event)
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut WindowManagerState,
        handle: &mut PointerInnerHandle<'_, WindowManagerState>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event)
    }

    fn start_data(&self) -> &PointerGrabStartData<WindowManagerState> {
        &self.start_data
    }

    fn unset(&mut self, _data: &mut WindowManagerState) {}
}
