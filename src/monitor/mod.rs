use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper,
        element::{surface::WaylandSurfaceRenderElement, utils::CropRenderElement},
    },
    desktop::{Window, WindowSurfaceType},
    output::Output,
    reexports::{
        calloop::timer::{TimeoutAction, Timer},
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::protocol::wl_surface::WlSurface,
    },
    utils::{Logical, Point, SERIAL_COUNTER, Scale},
};

use crate::{
    monitor::workspace::{TileResizeUnit, Workspace},
    state::WindowManagerState,
    utils::{Direction, get_app_id_and_title},
    window::{
        MappedWindow,
        rule::{
            WindowDynamicProperties, WindowLocation, WindowOpeningProperties, WindowProperties,
            WindowRuleCandidate, WindowState,
        },
    },
};

pub use record::LayoutRecord;
pub use workspace::{LayoutSet, TileRatio, TileTreeSearchKey, TileTreeWindow, WorkspaceName};

mod record;
mod workspace;

#[derive(Debug, Default)]
pub struct Monitors {
    monitors: Vec<Monitor>,
}

impl Monitors {
    pub fn push(&mut self, output: Output, layouts: Rc<LayoutSet>, layout_name: Rc<str>) {
        self.monitors
            .push(Monitor::new(output, layouts, layout_name));
    }

    pub fn get_monitor(&self) -> &Monitor {
        self.monitors.last().unwrap()
    }
    pub fn get_monitor_mut(&mut self) -> &mut Monitor {
        self.monitors.last_mut().unwrap()
    }
}

#[derive(Debug)]
pub struct Monitor {
    output: Output,

    active_workspace: WorkspaceName,
    workspaces: Vec<Workspace>,

    layouts: Rc<LayoutSet>,
    layout_name: Rc<str>,
}

impl Monitor {
    fn new(output: Output, layouts: Rc<LayoutSet>, layout_name: Rc<str>) -> Self {
        let active_workspace = WorkspaceName::Id(1);
        Self {
            output: output.clone(),
            active_workspace: active_workspace.clone(),
            workspaces: vec![Workspace::new(
                output,
                active_workspace,
                layouts.clone(),
                &layout_name,
            )],
            layouts,
            layout_name,
        }
    }

    pub fn get_active_workspace_name(&self) -> &WorkspaceName {
        &self.active_workspace
    }

    fn find_workspace_index(&self, workspace_name: &WorkspaceName) -> Option<usize> {
        #[allow(irrefutable_let_patterns)]
        let WorkspaceName::Id(id) = workspace_name else {
            return None;
        };
        self.workspaces
            .binary_search_by_key(id, |workspace| workspace.get_name().as_id())
            .ok()
    }

    pub fn get_workspace(&self, workspace_name: &WorkspaceName) -> &Workspace {
        if let Some(index) = self.find_workspace_index(workspace_name) {
            return &self.workspaces[index];
        }
        unreachable!()
    }

    pub fn get_workspace_mut(&mut self, workspace_name: &WorkspaceName) -> &mut Workspace {
        if let Some(index) = self.find_workspace_index(workspace_name) {
            return &mut self.workspaces[index];
        }
        unreachable!()
    }

    fn add_workspace(&mut self, workspace_name: WorkspaceName) {
        match workspace_name {
            WorkspaceName::Id(id) => {
                if let Err(index) = self.workspaces.binary_search_by_key(&id, |workspace| {
                    let WorkspaceName::Id(id) = workspace.get_name();
                    *id
                }) {
                    self.workspaces.insert(
                        index,
                        Workspace::new(
                            self.output.clone(),
                            workspace_name,
                            self.layouts.clone(),
                            &self.layout_name,
                        ),
                    );
                }
            }
        }
    }

    fn remove_workspace(&mut self, workspace_name: &WorkspaceName) -> Option<Workspace> {
        match workspace_name {
            WorkspaceName::Id(id) => {
                if let Ok(index) = self.workspaces.binary_search_by_key(&id, |workspace| {
                    let WorkspaceName::Id(id) = workspace.get_name();
                    id
                }) {
                    return Some(self.workspaces.remove(index));
                }
            }
        }

        None
    }
}

pub struct FoundMappedWindow {
    pub mapped: MappedWindow,
    pub workspace_name: WorkspaceName,
    pub location: Option<Point<i32, Logical>>,
}

impl WindowManagerState {
    pub fn add_window(&mut self, window: Window, properties: WindowProperties) {
        let WindowOpeningProperties {
            focus,
            state,
            workspace_name,
        } = properties.opening.unwrap();

        let focus = focus.unwrap_or(true);
        let floating = matches!(state, Some(WindowState::Float { .. }));
        let mut mapped = MappedWindow::new(window, focus, floating);

        let monitor = self.monitors.get_monitor_mut();
        let workspace_name = workspace_name
            .inspect(|name| {
                monitor.add_workspace(name.clone());
            })
            .unwrap_or(monitor.get_active_workspace_name().clone());
        if focus {
            monitor.active_workspace = workspace_name.clone();
        }
        let workspace = monitor.get_workspace_mut(&workspace_name);

        match state.unwrap_or_default() {
            WindowState::Float { location, size } => {
                let window_size = size
                    .map(|(w, h)| (w, h).into())
                    .unwrap_or(mapped.get_geometry_size());
                match location {
                    Some(WindowLocation::Location(x, y)) => {
                        mapped.set_location((x, y).into());
                    }
                    Some(WindowLocation::Center) => {
                        let output_size = &workspace.get_output_size();
                        mapped.set_location(
                            (
                                output_size.w / 2 - window_size.w / 2,
                                output_size.h / 2 - window_size.h / 2,
                            )
                                .into(),
                        );
                    }
                    _ => {}
                }
                mapped.set_size(mapped.clamp_size(window_size));

                apply_rule_to_mapped_window(&mapped, properties.dynamic);
                workspace.add_floating_window(mapped.clone());
            }
            WindowState::Tile { ratio } => {
                apply_rule_to_mapped_window(&mapped, properties.dynamic);
                let mapped = workspace.add_tiling_window(mapped.clone(), ratio);
                self.handle_tiling_layout_full(mapped.clone());
                if mapped.is_some() {
                    return;
                }
            }
        }

        if mapped.get_focus() {
            self.focus_window(&mapped.wl_surface());
        }

        self.timeout_to_save();
    }

    fn handle_tiling_layout_full(&mut self, mapped: Option<MappedWindow>) {
        let Some(mapped) = mapped else {
            return;
        };
        let (app_id, title) = get_app_id_and_title(&mapped.wl_surface());

        let candidate = WindowRuleCandidate {
            app_id,
            title,
            focus: true,
            float: true,
            workspace_name: self
                .monitors
                .get_monitor()
                .get_active_workspace_name()
                .clone(),
        };
        let mut properties = self.window_rules.get_properties(candidate, true);
        properties = properties.merge(WindowProperties {
            opening: Some(WindowOpeningProperties {
                state: Some(WindowState::Float {
                    location: None,
                    size: None,
                }),
                ..Default::default()
            }),
            ..Default::default()
        });

        self.add_window(mapped.window(), properties);
    }

    pub fn find_mapped_window(&self, surface: &WlSurface) -> Option<FoundMappedWindow> {
        self.monitors
            .get_monitor()
            .workspaces
            .iter()
            .find_map(|workspace| {
                workspace
                    .find_window(surface)
                    .map(|mapped| FoundMappedWindow {
                        mapped: mapped.clone(),
                        workspace_name: workspace.get_name().clone(),
                        location: None,
                    })
            })
    }

    pub fn remove_mapped_window(&mut self, surface: &WlSurface) -> Option<FoundMappedWindow> {
        self.monitors
            .get_monitor_mut()
            .workspaces
            .iter_mut()
            .find_map(|workspace| {
                let workspace_name = workspace.get_name().clone();
                workspace
                    .remove_window(surface)
                    .map(|mapped| FoundMappedWindow {
                        mapped,
                        workspace_name,
                        location: None,
                    })
            })
    }

    pub fn focus_window(&mut self, surface: &WlSurface) {
        let keyboard = self.get_keyboard();

        if let Some(old_surface) = keyboard.current_focus() {
            if old_surface == *surface {
                return;
            }
            if let Some(FoundMappedWindow {
                mapped,
                workspace_name,
                ..
            }) = self.find_mapped_window(&old_surface)
            {
                mapped.set_focus(false);
                mapped.window().set_activated(false);

                let (app_id, title) = get_app_id_and_title(&old_surface);
                let properties = self.window_rules.get_properties(
                    WindowRuleCandidate {
                        app_id,
                        title,
                        focus: mapped.get_focus(),
                        float: mapped.get_floating(),
                        workspace_name,
                    },
                    false,
                );
                apply_rule_to_mapped_window(&mapped, properties.dynamic);
            }
        }
        keyboard.set_focus(self, Some(surface.clone()), SERIAL_COUNTER.next_serial());
        let Some(FoundMappedWindow {
            mapped,
            workspace_name,
            ..
        }) = self.find_mapped_window(surface)
        else {
            return;
        };
        mapped.set_focus(true);
        mapped.window().set_activated(true);

        let (app_id, title) = get_app_id_and_title(surface);
        let properties = self.window_rules.get_properties(
            WindowRuleCandidate {
                app_id,
                title,
                focus: mapped.get_focus(),
                float: mapped.get_floating(),
                workspace_name: workspace_name.clone(),
            },
            false,
        );
        apply_rule_to_mapped_window(&mapped, properties.dynamic);

        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);
        workspace.append_to_focus_queue(mapped.clone());
        if mapped.get_floating() {
            workspace.raise_floating_window(surface);
        }
    }

    pub fn find_mapped_window_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<FoundMappedWindow> {
        let monitor = self.monitors.get_monitor();
        let workspace_name = monitor.get_active_workspace_name();
        monitor
            .get_workspace(workspace_name)
            .find_mapped_window_under(point)
            .map(|(mapped, location)| FoundMappedWindow {
                mapped: mapped.clone(),
                workspace_name: workspace_name.clone(),
                location: Some(location),
            })
    }

    pub fn find_surface_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.find_mapped_window_under(point).and_then(
            |FoundMappedWindow {
                 mapped, location, ..
             }| {
                let Some(location) = location else {
                    unreachable!()
                };
                mapped
                    .window()
                    .surface_under(point - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, (p + location).to_f64()))
            },
        )
    }

    pub fn restore_workspace_focus(&mut self, workspace_name: &WorkspaceName) {
        let monitor = self.monitors.get_monitor_mut();
        if let Some(mapped) = monitor
            .get_workspace(workspace_name)
            .get_last_focused_window()
            .cloned()
        {
            self.focus_window(&mapped.wl_surface());
        } else {
            self.get_keyboard()
                .set_focus(self, None, SERIAL_COUNTER.next_serial());
        }
    }

    pub fn switch_or_create_active_workspace(&mut self, workspace_name: WorkspaceName) {
        let monitor = self.monitors.get_monitor_mut();

        let old_workspace_name = monitor.get_active_workspace_name().clone();
        let old_workspace = monitor.get_workspace(&old_workspace_name);
        // TODO improve efficiency
        if old_workspace.windows_iter().count() == 0 {
            monitor.remove_workspace(&old_workspace_name);
        }

        monitor.add_workspace(workspace_name.clone());
        monitor.active_workspace = workspace_name.clone();

        self.restore_workspace_focus(&workspace_name);
    }

    fn goto_workspace_by_delta(&mut self, delta: i32) {
        let monitor = self.monitors.get_monitor_mut();

        let old_workspace_name = monitor.get_active_workspace_name();
        let Some(index) = monitor.find_workspace_index(old_workspace_name) else {
            return;
        };

        let len = monitor.workspaces.len() as i32;
        let new_index = (index as i32 + delta).rem_euclid(len) as usize;

        let new_workspace_name = monitor.workspaces[new_index].get_name().clone();
        monitor.active_workspace = new_workspace_name.clone();
        self.restore_workspace_focus(&new_workspace_name);
    }

    pub fn goto_next_workspace(&mut self) {
        self.goto_workspace_by_delta(1);
    }

    pub fn goto_prev_workspace(&mut self) {
        self.goto_workspace_by_delta(-1);
    }

    pub fn move_focused_window_to_workspace(&mut self, workspace_name: WorkspaceName, focus: bool) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some(FoundMappedWindow {
            mapped,
            workspace_name: old_workspace_name,
            ..
        }) = self.remove_mapped_window(&surface)
        else {
            return;
        };

        if !focus {
            self.restore_workspace_focus(&old_workspace_name);
        }

        let monitor = self.monitors.get_monitor_mut();
        monitor.add_workspace(workspace_name.clone());
        if focus {
            monitor.active_workspace = workspace_name.clone();
        }
        let workspace = monitor.get_workspace_mut(&workspace_name);

        let (app_id, title) = get_app_id_and_title(&mapped.wl_surface());
        let properties = self.window_rules.get_properties(
            WindowRuleCandidate {
                app_id,
                title,
                focus,
                float: mapped.get_floating(),
                workspace_name: workspace_name.clone(),
            },
            false,
        );
        apply_rule_to_mapped_window(&mapped, properties.dynamic);

        if mapped.get_floating() {
            workspace.add_floating_window(mapped.clone());
        } else {
            let mapped = workspace.add_tiling_window(mapped.clone(), None);
            self.handle_tiling_layout_full(mapped);
        }

        if focus {
            self.focus_window(&mapped.wl_surface());
        }

        self.timeout_to_save();
    }

    pub fn toggle_focused_window_floating(&mut self) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some(FoundMappedWindow {
            mapped,
            workspace_name,
            ..
        }) = self.remove_mapped_window(&surface)
        else {
            return;
        };

        mapped.set_floating(!mapped.get_floating());

        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);

        let (app_id, title) = get_app_id_and_title(&mapped.wl_surface());
        let properties = self.window_rules.get_properties(
            WindowRuleCandidate {
                app_id,
                title,
                focus: true,
                float: mapped.get_floating(),
                workspace_name: workspace_name.clone(),
            },
            false,
        );
        apply_rule_to_mapped_window(&mapped, properties.dynamic);

        if mapped.get_floating() {
            workspace.add_floating_window(mapped);
        } else {
            let mapped = workspace.add_tiling_window(mapped, None);
            self.handle_tiling_layout_full(mapped);
        }

        self.timeout_to_save();
    }

    pub fn close_focused_window(&mut self) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some(FoundMappedWindow { mapped, .. }) = self.find_mapped_window(&surface) else {
            return;
        };

        mapped.toplevel().send_close();

        self.timeout_to_save();
    }

    pub fn focus_tiling_window_in_direction(&mut self, direction: Direction) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some(FoundMappedWindow {
            mapped,
            workspace_name,
            ..
        }) = self.find_mapped_window(&surface)
        else {
            return;
        };
        if mapped.get_floating() {
            return;
        }
        let monitor = self.monitors.get_monitor();
        let workspace = monitor.get_workspace(&workspace_name);
        if let Some(mapped) = workspace.last_focused_tiling_window_in_direction(&surface, direction)
        {
            self.focus_window(&mapped.wl_surface());
        }
    }

    pub fn swap_tiling_window(&mut self, lhs: &WlSurface, rhs: &WlSurface) {
        let Some(FoundMappedWindow {
            mapped: mapped_lhs,
            workspace_name,
            ..
        }) = self.find_mapped_window(lhs)
        else {
            return;
        };
        let Some(FoundMappedWindow {
            mapped: mapped_rhs, ..
        }) = self.find_mapped_window(rhs)
        else {
            return;
        };
        if mapped_lhs.get_floating() || mapped_rhs.get_floating() {
            return;
        }

        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);
        workspace.swap_tiling_window(lhs, rhs);

        self.timeout_to_save();
    }

    pub fn swap_focused_tiling_window_in_direction(&mut self, direction: Direction) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some(FoundMappedWindow {
            mapped: mapped_lhs,
            workspace_name,
            ..
        }) = self.find_mapped_window(&surface)
        else {
            return;
        };
        if mapped_lhs.get_floating() {
            return;
        }

        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);
        if let Some(mapped_rhs) = workspace
            .last_focused_tiling_window_in_direction(&surface, direction)
            .cloned()
        {
            workspace.swap_tiling_window(&mapped_lhs.wl_surface(), &mapped_rhs.wl_surface());
        }

        self.timeout_to_save();
    }

    pub fn resize_focused_tiling_window(
        &mut self,
        direction: Direction,
        unit: impl Into<TileResizeUnit>,
    ) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some(FoundMappedWindow {
            mapped,
            workspace_name,
            ..
        }) = self.find_mapped_window(&surface)
        else {
            return;
        };
        if mapped.get_floating() {
            return;
        }
        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);
        workspace.resize_tiling_window(&surface, direction, unit);
    }

    pub fn apply_rule_to_mapped_windows(&mut self) {
        let monitor = self.monitors.get_monitor_mut();
        monitor.workspaces.iter_mut().for_each(|workspace| {
            workspace.apply_rule_to_windows(&self.window_rules);
        });
    }

    pub fn windows_in_active_workspace_iter(&self) -> impl Iterator<Item = &MappedWindow> {
        let monitor = self.monitors.get_monitor();
        monitor
            .get_workspace(monitor.get_active_workspace_name())
            .windows_iter()
    }

    pub fn render_elements<R>(
        &mut self,
        renderer: &mut R,
        scale: Scale<f64>,
    ) -> Vec<CropRenderElement<WaylandSurfaceRenderElement<R>>>
    where
        R: Renderer + ImportAll,
        <R as RendererSuper>::TextureId: Clone + 'static,
    {
        let monitor = self.monitors.get_monitor_mut();
        monitor
            .get_workspace_mut(&monitor.get_active_workspace_name().clone())
            .render_elements(renderer, scale)
    }

    fn timeout_to_save(&mut self) {
        let is_none = self.save_at.is_none();
        let time = Instant::now() + Duration::from_secs(5);
        self.save_at = Some(time);
        if is_none {
            // TODO
            let _ = self
                .event_loop
                .insert_source(Timer::from_deadline(time), |_, _, data| {
                    let data = &mut data.compositor;

                    let Some(time) = data.save_at else {
                        unreachable!()
                    };
                    if time <= Instant::now() {
                        let mut ls = Vec::new();
                        for w in &data.monitors.get_monitor().workspaces {
                            let items = w.tiling_windows_iter().cloned().collect();
                            ls.push(items);
                        }
                        for l in ls {
                            data.append_layout_record(l);
                        }

                        data.save_at = None;
                        TimeoutAction::Drop
                    } else {
                        data.save_at = Some(time);
                        TimeoutAction::ToInstant(time)
                    }
                });
        }
    }
}

pub fn apply_rule_to_mapped_window(mapped: &MappedWindow, properties: WindowDynamicProperties) {
    if mapped.get_floating() {
        mapped.toplevel().with_pending_state(|state| {
            use xdg_toplevel::State::*;
            state.states.unset(TiledTop);
            state.states.unset(TiledBottom);
            state.states.unset(TiledLeft);
            state.states.unset(TiledRight);
        });
    } else {
        mapped.toplevel().with_pending_state(|state| {
            use xdg_toplevel::State::*;
            state.states.set(TiledTop);
            state.states.set(TiledBottom);
            state.states.set(TiledLeft);
            state.states.set(TiledRight);
        });
    }
    mapped.toplevel().with_pending_state(|state| {
        state.decoration_mode = properties.decoration.map(|d| d.into());
    });

    if let Some(opacity) = properties.opacity {
        mapped.set_opacity(opacity);
    }
}
