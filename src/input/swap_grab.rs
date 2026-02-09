use crate::{
    monitor::apply_rule_to_mapped_window, state::WindowManagerState,
    window::rule::WindowDynamicProperties,
};
use smithay::{
    desktop::Window,
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
};

pub struct SwapGrab {
    start_data: PointerGrabStartData<WindowManagerState>,
    window: Window,
    last_window: Option<(Window, WindowDynamicProperties)>,
}

impl SwapGrab {
    pub fn new(start_data: PointerGrabStartData<WindowManagerState>, window: Window) -> Self {
        Self {
            start_data,
            window,
            last_window: None,
        }
    }
}

impl PointerGrab<WindowManagerState> for SwapGrab {
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

        let props = data.pointer_config.selection.clone();
        if let Some((mapped, _)) = data.find_mapped_window_mut_under(event.location) {
            if self
                .last_window
                .as_ref()
                .is_some_and(|(w, _)| *w == mapped.window)
            {
                return;
            }

            let last_window = self.last_window.clone();

            if self.window == mapped.window {
                self.last_window = None;
            } else {
                self.last_window = Some((
                    mapped.window.clone(),
                    WindowDynamicProperties {
                        decoration: mapped
                            .toplevel()
                            .current_state()
                            .decoration_mode
                            .map(|m| m.into()),
                        opacity: Some(mapped.opacity),
                    },
                ));
                apply_rule_to_mapped_window(mapped, props);
            }

            if let Some((window, properties)) = last_window
                && let Some((mapped, _)) =
                    data.find_mapped_window_mut(window.toplevel().unwrap().wl_surface())
            {
                apply_rule_to_mapped_window(mapped, properties);
            }
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
            if let Some((window, properties)) = self.last_window.clone()
                && let Some((mapped, _)) =
                    data.find_mapped_window_mut(window.toplevel().unwrap().wl_surface())
            {
                apply_rule_to_mapped_window(mapped, properties);
            }
            if let Some((window, _)) = &self.last_window
                && *window != self.window
            {
                data.swap_window(
                    self.window.toplevel().unwrap().wl_surface(),
                    window.toplevel().unwrap().wl_surface(),
                );
            }

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
