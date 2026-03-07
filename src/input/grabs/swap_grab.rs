use crate::{
    monitor::{FoundMappedWindow, WorkspaceName},
    state::WindowManagerState,
    utils::{apply_rule_to_mapped_window, get_app_id_and_title},
    window::{MappedWindow, rule::WindowRuleCandidate},
};
use smithay::input::{
    SeatHandler,
    pointer::{
        AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
        GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent,
        GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData as PointerGrabStartData,
        MotionEvent, PointerGrab, PointerInnerHandle, RelativeMotionEvent,
    },
};

pub struct SwapGrab {
    start_data: PointerGrabStartData<WindowManagerState>,
    mapped: MappedWindow,
    workspace_name: WorkspaceName,
    last_mapped: Option<MappedWindow>,
    hidden: bool,
}

impl SwapGrab {
    pub fn new(
        start_data: PointerGrabStartData<WindowManagerState>,
        mapped: MappedWindow,
        workspace_name: WorkspaceName,
        hidden: bool,
    ) -> Self {
        Self {
            start_data,
            mapped,
            workspace_name,
            last_mapped: None,
            hidden,
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

        if let Some(FoundMappedWindow {
            mapped,
            workspace_name,
            ..
        }) = data.find_mapped_window_under(event.location)
        {
            if self.last_mapped.as_ref().is_some_and(|w| *w == mapped) {
                return;
            }

            if let Some(mapped) = &self.last_mapped {
                let (app_id, title) = get_app_id_and_title(&mapped.wl_surface());
                let properties = data.window_rules.get_properties(
                    WindowRuleCandidate {
                        app_id,
                        title,
                        focus: mapped.get_focus(),
                        float: mapped.get_floating(),
                        is_swap_source: false,
                        is_swap_target: false,
                        workspace_name: workspace_name.clone(),
                    },
                    false,
                );
                apply_rule_to_mapped_window(mapped, properties.dynamic);
            }

            if self.mapped == mapped {
                self.last_mapped = None;
            } else {
                let (app_id, title) = get_app_id_and_title(&mapped.wl_surface());
                let properties = data.window_rules.get_properties(
                    WindowRuleCandidate {
                        app_id,
                        title,
                        focus: mapped.get_focus(),
                        float: mapped.get_floating(),
                        is_swap_source: false,
                        is_swap_target: true,
                        workspace_name,
                    },
                    false,
                );
                apply_rule_to_mapped_window(&mapped, properties.dynamic);
                self.last_mapped = Some(mapped.clone());
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
            if let Some(mapped) = self.last_mapped.as_ref() {
                let (app_id, title) = get_app_id_and_title(&mapped.wl_surface());
                let properties = data.window_rules.get_properties(
                    WindowRuleCandidate {
                        app_id,
                        title,
                        focus: mapped.get_focus(),
                        float: mapped.get_floating(),
                        is_swap_source: false,
                        is_swap_target: false,
                        workspace_name: self.workspace_name.clone(),
                    },
                    false,
                );
                apply_rule_to_mapped_window(mapped, properties.dynamic);
                data.swap_tiling_window(&mapped.wl_surface(), &self.mapped.wl_surface());
            }
            let (app_id, title) = get_app_id_and_title(&self.mapped.wl_surface());
            let properties = data.window_rules.get_properties(
                WindowRuleCandidate {
                    app_id,
                    title,
                    focus: self.mapped.get_focus(),
                    float: self.mapped.get_floating(),
                    is_swap_source: false,
                    is_swap_target: false,
                    workspace_name: self.workspace_name.clone(),
                },
                false,
            );
            apply_rule_to_mapped_window(&self.mapped, properties.dynamic);
            data.set_focused_workspace_floating_window_hidden(Some(self.hidden), Some(false));
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
