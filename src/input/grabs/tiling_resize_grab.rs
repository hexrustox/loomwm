// TODO set xdg resizing state
use crate::{state::WindowManagerState, utils::Direction};
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
    utils::{Logical, Size},
};

pub struct TilingResizeGrab {
    start_data: PointerGrabStartData<WindowManagerState>,
    direction: Direction,
    initial_size: Size<i32, Logical>,
}

impl TilingResizeGrab {
    pub fn new(
        start_data: PointerGrabStartData<WindowManagerState>,
        direction: Direction,
        initial_size: Size<i32, Logical>,
    ) -> Self {
        Self {
            start_data,
            direction,
            initial_size,
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

        if self
            .direction
            .intersects(Direction::LEFT | Direction::RIGHT)
        {
            let adjusted_x = if self.direction.intersects(Direction::LEFT) {
                -delta.x
            } else {
                delta.x
            };

            let new_width = (self.initial_size.w as f64 + adjusted_x) as i32;

            data.resize_focused_tiling_window(
                self.direction
                    .intersection(Direction::LEFT | Direction::RIGHT),
                new_width,
            );
        }

        if self
            .direction
            .intersects(Direction::TOP | Direction::BOTTOM)
        {
            let adjusted_y = if self.direction.intersects(Direction::TOP) {
                -delta.y
            } else {
                delta.y
            };

            let new_height = (self.initial_size.h as f64 + adjusted_y) as i32;

            data.resize_focused_tiling_window(
                self.direction
                    .intersection(Direction::TOP | Direction::BOTTOM),
                new_height,
            );
        }
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
