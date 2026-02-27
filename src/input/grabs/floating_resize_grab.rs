use smithay::{
    input::pointer::{
        AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
        GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent,
        GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData as PointerGrabStartData,
        MotionEvent, PointerGrab, PointerInnerHandle, RelativeMotionEvent,
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::protocol::wl_surface::WlSurface,
    },
    utils::{Logical, Point, Rectangle, Size},
};

use crate::{
    monitor::TileTreeWindow, state::WindowManagerState, utils::Direction, window::MappedWindow,
};

impl From<xdg_toplevel::ResizeEdge> for Direction {
    fn from(x: xdg_toplevel::ResizeEdge) -> Self {
        Self::from_bits(x as u32).unwrap()
    }
}

pub struct FloatingResizeGrab {
    start_data: PointerGrabStartData<WindowManagerState>,
    mapped: MappedWindow,

    direction: Direction,

    initial_rect: Rectangle<i32, Logical>,
    last_size: Size<i32, Logical>,
}

impl FloatingResizeGrab {
    pub fn new(
        start_data: PointerGrabStartData<WindowManagerState>,
        mapped: MappedWindow,
        direction: Direction,
        initial_rect: Rectangle<i32, Logical>,
    ) -> Self {
        mapped.set_resize_state(ResizeGrabState::Resizing {
            direction,
            initial_rect,
        });
        Self {
            start_data,
            mapped,
            direction,
            initial_rect,
            last_size: initial_rect.size,
        }
    }
}

impl PointerGrab<WindowManagerState> for FloatingResizeGrab {
    fn motion(
        &mut self,
        data: &mut WindowManagerState,
        handle: &mut PointerInnerHandle<'_, WindowManagerState>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        handle.motion(data, None, event);

        let mut delta = event.location - self.start_data.location;

        let mut new_width = self.initial_rect.size.w;
        let mut new_height = self.initial_rect.size.h;

        if self
            .direction
            .intersects(Direction::LEFT | Direction::RIGHT)
        {
            if self.direction.intersects(Direction::LEFT) {
                delta.x = -delta.x;
            }

            new_width = (self.initial_rect.size.w as f64 + delta.x) as i32;
        }

        if self
            .direction
            .intersects(Direction::TOP | Direction::BOTTOM)
        {
            if self.direction.intersects(Direction::TOP) {
                delta.y = -delta.y;
            }

            new_height = (self.initial_rect.size.h as f64 + delta.y) as i32;
        }

        self.last_size = Size::from((new_width, new_height));

        self.mapped.set_size(self.mapped.clamp_size(self.last_size));
        self.mapped.toplevel().with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Resizing);
        });
    }

    fn relative_motion(
        &mut self,
        data: &mut WindowManagerState,
        handle: &mut PointerInnerHandle<'_, WindowManagerState>,
        focus: Option<(WlSurface, Point<f64, Logical>)>,
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

            self.mapped.set_size(self.mapped.clamp_size(self.last_size));
            self.mapped.toplevel().with_pending_state(|state| {
                state.states.unset(xdg_toplevel::State::Resizing);
            });

            self.mapped
                .set_resize_state(ResizeGrabState::WaitingForLastCommit {
                    direction: self.direction,
                    initial_rect: self.initial_rect,
                });
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

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum ResizeGrabState {
    #[default]
    Idle,
    Resizing {
        direction: Direction,
        initial_rect: Rectangle<i32, Logical>,
    },
    WaitingForLastCommit {
        direction: Direction,
        initial_rect: Rectangle<i32, Logical>,
    },
}

impl ResizeGrabState {
    pub fn commit(&mut self) -> Option<(Direction, Rectangle<i32, Logical>)> {
        match *self {
            Self::Resizing {
                direction,
                initial_rect,
            } => Some((direction, initial_rect)),
            Self::WaitingForLastCommit {
                direction,
                initial_rect,
            } => {
                *self = Self::Idle;
                Some((direction, initial_rect))
            }
            Self::Idle => None,
        }
    }
}
