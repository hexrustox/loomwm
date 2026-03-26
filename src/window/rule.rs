use regex::Regex;
use serde::Deserialize;
use smithay::reexports::{
    wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode,
    wayland_server::protocol::wl_surface::WlSurface,
};
use tracing::error;

use crate::{
    monitor::{TileRatio, WorkspaceName},
    utils::{RGBAColor, get_app_id_and_title},
    window::{MappedWindow, WindowRole},
};

#[derive(Debug, Default, Deserialize)]
#[serde(transparent)]
pub struct WindowRules(Vec<WindowRule>);

impl WindowRules {
    pub fn get_opening_properties(
        &self,
        surface: &WlSurface,
        workspace_name: WorkspaceName,
        float_state: Option<WindowState>,
    ) -> WindowProperties {
        let (app_id, title) = get_app_id_and_title(surface);

        let default_state = Some(WindowState::Tile { ratio: None });
        let layout_state = float_state.or(default_state);

        let mut candidate = WindowRuleCandidate {
            app_id,
            title,
            is_focused: true,
            is_floating: matches!(layout_state, Some(WindowState::Float { .. })),
            role: WindowRole::Normal,
            workspace_name,
        };
        let mut properties = WindowProperties {
            opening: Some(WindowOpeningProperties {
                open_with_focus: Some(true),
                layout_state,
                ..Default::default()
            }),
            dynamic: WindowDynamicProperties::default(),
        };

        for rule in &self.0 {
            if rule.is_match(&candidate) {
                properties = properties.merge(rule.properties.clone());

                if let Some(WindowOpeningProperties {
                    open_with_focus: Some(focus),
                    ..
                }) = properties.opening
                {
                    candidate.is_focused = focus;
                }

                if let Some(WindowOpeningProperties {
                    layout_state: Some(WindowState::Float { .. }),
                    ..
                }) = properties.opening
                {
                    candidate.is_floating = true;
                }

                if let Some(WindowOpeningProperties {
                    open_as: Some(role),
                    ..
                }) = properties.opening
                {
                    candidate.role = role;
                }

                if let Some(WindowOpeningProperties {
                    open_in_workspace: Some(ref workspace_name),
                    ..
                }) = properties.opening
                {
                    candidate.workspace_name = workspace_name.clone();
                }
            }
        }

        properties
    }

    pub fn get_dynamic_properties(
        &self,
        mapped: &MappedWindow,
        workspace_name: WorkspaceName,
    ) -> WindowDynamicProperties {
        let (app_id, title) = get_app_id_and_title(&mapped.wl_surface());

        let candidate = WindowRuleCandidate {
            app_id,
            title,
            is_focused: mapped.is_focused(),
            is_floating: mapped.is_floating(),
            role: mapped.role(),
            workspace_name,
        };

        let mut properties = WindowProperties {
            opening: None,
            dynamic: WindowDynamicProperties::default(),
        };

        for rule in &self.0 {
            if rule.is_match(&candidate) {
                properties = properties.merge(rule.properties.clone());
            }
        }

        properties.dynamic
    }
}

#[derive(Debug, Deserialize)]
struct WindowRule {
    #[serde(default = "default_window_rule_matches")]
    matches: Vec<WindowRuleMatch>,
    #[serde(flatten)]
    properties: WindowProperties,
}

fn default_window_rule_matches() -> Vec<WindowRuleMatch> {
    vec![WindowRuleMatch::default()]
}

impl WindowRule {
    fn is_match(&self, candidate: &WindowRuleCandidate) -> bool {
        fn regex_matches(str: Option<&str>, haystack: &str) -> bool {
            if let Some(re) = str {
                if let Ok(re) = Regex::new(re) {
                    return re.is_match(haystack);
                } else {
                    error!("Invalid regex: {re}");
                }
            }
            true
        }

        self.matches.iter().any(|rule| {
            regex_matches(rule.app_id_pattern.as_deref(), &candidate.app_id)
                && regex_matches(rule.title_pattern.as_deref(), &candidate.title)
                && rule.is_focused.is_none_or(|v| candidate.is_focused == v)
                && rule.is_floating.is_none_or(|v| candidate.is_floating == v)
                && rule.role.is_none_or(|v| candidate.role == v)
                && rule
                    .in_workspace
                    .as_ref()
                    .is_none_or(|v| candidate.workspace_name == *v)
        })
    }
}

#[derive(Debug, Default)]
pub struct WindowRuleMatch {
    pub app_id_pattern: Option<String>,
    pub title_pattern: Option<String>,
    pub is_focused: Option<bool>,
    pub is_floating: Option<bool>,
    pub role: Option<WindowRole>,
    pub in_workspace: Option<WorkspaceName>,
}

#[derive(Debug, Clone)]
struct WindowRuleCandidate {
    app_id: String,
    title: String,
    is_focused: bool,
    is_floating: bool,
    role: WindowRole,
    workspace_name: WorkspaceName,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct WindowProperties {
    #[serde(flatten)]
    pub opening: Option<WindowOpeningProperties>,
    #[serde(default, flatten)]
    pub dynamic: WindowDynamicProperties,
}

impl Default for WindowProperties {
    fn default() -> Self {
        Self {
            opening: Some(WindowOpeningProperties::default()),
            dynamic: WindowDynamicProperties::default(),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct WindowOpeningProperties {
    pub open_with_focus: Option<bool>,
    #[serde(flatten)]
    pub layout_state: Option<WindowState>,
    pub open_as: Option<WindowRole>,
    pub open_in_workspace: Option<WorkspaceName>,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
pub struct WindowDynamicProperties {
    pub decoration: Option<WindowDecoration>,
    pub border: Option<WindowBorder>,
    pub opacity: Option<f32>,
}

impl WindowProperties {
    pub fn merge(self, other: Self) -> Self {
        Self {
            opening: if let Some(rhs) = other.opening {
                self.opening.map(|opening| opening.override_with(rhs))
            } else {
                self.opening
            },
            dynamic: self.dynamic.override_with(other.dynamic),
        }
    }
}

impl WindowOpeningProperties {
    fn override_with(self, other: Self) -> Self {
        Self {
            open_with_focus: other.open_with_focus.or(self.open_with_focus),
            layout_state: other.layout_state.or(self.layout_state),
            open_as: other.open_as.or(self.open_as),
            open_in_workspace: other.open_in_workspace.or(self.open_in_workspace),
        }
    }
}

impl WindowDynamicProperties {
    fn override_with(self, other: Self) -> Self {
        Self {
            decoration: other.decoration.or(self.decoration),
            border: other.border.or(self.border),
            opacity: other.opacity.or(self.opacity),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WindowDecoration {
    #[default]
    ClientSide,
    ServerSide,
}

impl From<WindowDecoration> for Mode {
    fn from(value: WindowDecoration) -> Self {
        match value {
            WindowDecoration::ClientSide => Mode::ClientSide,
            WindowDecoration::ServerSide => Mode::ServerSide,
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
pub struct WindowBorder {
    pub width: u32,
    pub color: RGBAColor,
}

type N = i32;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum WindowState {
    #[serde(rename = "open-as-floating")]
    Float {
        location: Option<WindowLocation>,
        size: Option<(N, N)>,
    },
    #[serde(rename = "open-as-tiling")]
    Tile { ratio: Option<TileRatio> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowLocation {
    Center,
    Location(N, N),
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    impl Default for WindowRuleCandidate {
        fn default() -> Self {
            Self {
                app_id: String::new(),
                title: String::new(),
                is_focused: false,
                is_floating: false,
                role: WindowRole::Normal,
                workspace_name: WorkspaceName::Id(0),
            }
        }
    }

    #[test_case(
        vec![WindowRuleMatch::default()],
        WindowRuleCandidate { role: WindowRole::Maximized, ..Default::default() } => true;
        "none_role_matches_any"
    )]
    #[test_case(
        vec![WindowRuleMatch::default()],
        WindowRuleCandidate { role: WindowRole::Normal, ..Default::default() } => true;
        "none_role_matches_normal"
    )]
    #[test_case(
        vec![WindowRuleMatch { role: Some(WindowRole::Maximized), ..Default::default() }],
        WindowRuleCandidate { role: WindowRole::Maximized, ..Default::default() } => true;
        "exact_match_maximized"
    )]
    #[test_case(
        vec![WindowRuleMatch { role: Some(WindowRole::Maximized), ..Default::default() }],
        WindowRuleCandidate { role: WindowRole::Normal, ..Default::default() } => false;
        "mismatch_maximized_vs_normal"
    )]
    #[test_case(
        vec![WindowRuleMatch { role: Some(WindowRole::Maximized), ..Default::default() }],
        WindowRuleCandidate { role: WindowRole::Fullscreen, ..Default::default() } => false;
        "mismatch_maximized_vs_fullscreen"
    )]
    #[test_case(
        vec![WindowRuleMatch { role: Some(WindowRole::Normal), ..Default::default() }],
        WindowRuleCandidate { role: WindowRole::Normal, ..Default::default() } => true;
        "exact_match_normal"
    )]
    #[test_case(
        vec![WindowRuleMatch { role: Some(WindowRole::SwapSource), ..Default::default() }],
        WindowRuleCandidate { role: WindowRole::SwapSource, ..Default::default() } => true;
        "exact_match_swap_source"
    )]
    #[test_case(
        vec![WindowRuleMatch { is_floating: Some(true), role: Some(WindowRole::Maximized), ..Default::default() }],
        WindowRuleCandidate { is_floating: true, role: WindowRole::Maximized, ..Default::default() } => true;
        "floating_and_role_both_match"
    )]
    #[test_case(
        vec![WindowRuleMatch { is_floating: Some(true), role: Some(WindowRole::Maximized), ..Default::default() }],
        WindowRuleCandidate { is_floating: true, role: WindowRole::Normal, ..Default::default() } => false;
        "floating_matches_but_role_does_not"
    )]
    fn test_role_match(matches: Vec<WindowRuleMatch>, candidate: WindowRuleCandidate) -> bool {
        (WindowRule {
            matches,
            properties: WindowProperties::default(),
        })
        .is_match(&candidate)
    }

    #[test_case(
        None, None => None;
        "none_none"
    )]
    #[test_case(
        None, Some(WindowRole::Maximized) => Some(WindowRole::Maximized);
        "none_some"
    )]
    #[test_case(
        Some(WindowRole::Maximized), None => Some(WindowRole::Maximized);
        "some_none"
    )]
    #[test_case(
        Some(WindowRole::Maximized), Some(WindowRole::Fullscreen) => Some(WindowRole::Fullscreen);
        "some_some_rhs_wins"
    )]
    fn test_merge_open_as(lhs: Option<WindowRole>, rhs: Option<WindowRole>) -> Option<WindowRole> {
        let lhs = WindowOpeningProperties {
            open_as: lhs,
            ..Default::default()
        };
        let rhs = WindowOpeningProperties {
            open_as: rhs,
            ..Default::default()
        };
        lhs.override_with(rhs).open_as
    }
}
