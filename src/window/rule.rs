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
    window::MappedWindow,
};

#[derive(Debug, Default, Deserialize)]
#[serde(transparent)]
pub struct WindowRules(Vec<WindowRule>);

impl WindowRules {
    pub fn get_opening_properties(
        &self,
        surface: &WlSurface,
        workspace_name: WorkspaceName,
        float: bool,
    ) -> WindowProperties {
        let (app_id, title) = get_app_id_and_title(surface);

        let mut properties = WindowProperties::default();
        let mut candidate = WindowRuleCandidate {
            app_id,
            title,
            focus: true,
            float,
            is_swap_source: false,
            is_swap_target: false,
            workspace_name: workspace_name.clone(),
        };

        for rule in &self.0 {
            if rule.is_match(&candidate) {
                properties = properties.merge(rule.properties.clone());

                // REMIND
                #[cfg(test)]
                if let Some(ref op) = properties.opening {
                    let WindowOpeningProperties {
                        focus: _,
                        state: _,
                        workspace_name: _,
                    } = op;
                }

                if let Some(WindowOpeningProperties {
                    focus: Some(focus), ..
                }) = properties.opening
                {
                    candidate.focus = focus;
                }
                if let Some(WindowOpeningProperties {
                    state: Some(WindowState::Float { .. }),
                    ..
                }) = properties.opening
                {
                    candidate.float = true;
                }
                if let Some(WindowOpeningProperties {
                    workspace_name: Some(ref workspace_name),
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
            focus: mapped.get_focus(),
            float: mapped.get_floating(),
            is_swap_source: mapped.get_is_swap_source(),
            is_swap_target: mapped.get_is_swap_target(),
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
            // REMIND
            #[cfg(test)]
            {
                let WindowRuleMatch {
                    app_id: _,
                    title: _,
                    focus: _,
                    float: _,
                    is_swap_source: _,
                    is_swap_target: _,
                    workspace_name: _,
                } = rule;
                let WindowRuleCandidate {
                    app_id: _,
                    title: _,
                    focus: _,
                    float: _,
                    is_swap_source: _,
                    is_swap_target: _,
                    workspace_name: _,
                } = candidate;
            }

            regex_matches(rule.app_id.as_deref(), &candidate.app_id)
                && regex_matches(rule.title.as_deref(), &candidate.title)
                && rule.focus.is_none_or(|v| candidate.focus == v)
                && rule.float.is_none_or(|v| candidate.float == v)
                && rule
                    .is_swap_source
                    .is_none_or(|v| candidate.is_swap_source == v)
                && rule
                    .is_swap_target
                    .is_none_or(|v| candidate.is_swap_target == v)
                && rule
                    .workspace_name
                    .as_ref()
                    .is_none_or(|v| candidate.workspace_name == *v)
        })
    }
}

// TODO tags
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct WindowRuleMatch {
    app_id: Option<String>,
    title: Option<String>,
    #[serde(rename = "is-focused")]
    focus: Option<bool>,
    #[serde(rename = "is-floating")]
    float: Option<bool>,
    is_swap_source: Option<bool>,
    is_swap_target: Option<bool>,
    #[serde(rename = "in-workspace")]
    workspace_name: Option<WorkspaceName>,
}

#[derive(Debug, Clone)]
pub(crate) struct WindowRuleCandidate {
    app_id: String,
    title: String,
    focus: bool,
    float: bool,
    is_swap_source: bool,
    is_swap_target: bool,
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
pub struct WindowOpeningProperties {
    #[serde(rename = "open-with-focus")]
    pub focus: Option<bool>,
    #[serde(flatten)]
    pub state: Option<WindowState>,
    #[serde(rename = "open-in-workspace")]
    pub workspace_name: Option<WorkspaceName>,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
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
            focus: other.focus.or(self.focus),
            state: other.state.or(self.state),
            workspace_name: other.workspace_name.or(self.workspace_name),
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

impl Default for WindowState {
    fn default() -> Self {
        Self::Tile { ratio: None }
    }
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
                app_id: "".into(),
                title: "".into(),
                focus: false,
                float: false,
                is_swap_source: false,
                is_swap_target: false,
                workspace_name: WorkspaceName::Id(0),
            }
        }
    }

    #[test_case(vec![], WindowRuleCandidate::default() => false; "empty")]
    #[test_case(
        vec![WindowRuleMatch::default()],
        WindowRuleCandidate::default() => true;
        "default_match"
    )]
    #[test_case(
        vec![WindowRuleMatch { float: Some(true), ..Default::default() }, WindowRuleMatch::default()],
        WindowRuleCandidate::default() => true;
        "fallback_to_second_rule"
    )]
    #[test_case(
        vec![WindowRuleMatch { app_id: Some("foo".into()), ..Default::default() }],
        WindowRuleCandidate { app_id: "foobar".into(), ..Default::default() } => true;
        "app_id_match"
    )]
    #[test_case(
        vec![WindowRuleMatch { app_id: Some("foo".into()), ..Default::default() }],
        WindowRuleCandidate::default() => false;
        "app_id_mismatch"
    )]
    #[test_case(
        vec![WindowRuleMatch { title: Some("test".into()), ..Default::default() }],
        WindowRuleCandidate { title: "my test window".into(), ..Default::default() } => true;
        "title_match"
    )]
    #[test_case(
        vec![WindowRuleMatch { focus: Some(true), ..Default::default() }],
        WindowRuleCandidate { focus: true, ..Default::default() } => true;
        "focus_match"
    )]
    #[test_case(
        vec![WindowRuleMatch { focus: Some(true), ..Default::default() }],
        WindowRuleCandidate::default() => false;
        "focus_mismatch"
    )]
    #[test_case(
        vec![WindowRuleMatch { float: Some(true), ..Default::default() }],
        WindowRuleCandidate { float: true, ..Default::default() } => true;
        "float_match"
    )]
    #[test_case(
        vec![WindowRuleMatch { float: Some(true), ..Default::default() }],
        WindowRuleCandidate::default() => false;
        "float_mismatch"
    )]
    #[test_case(
        vec![WindowRuleMatch { is_swap_source: Some(true), ..Default::default() }],
        WindowRuleCandidate { is_swap_source: true, ..Default::default() } => true;
        "is_swap_source_match"
    )]
    #[test_case(
        vec![WindowRuleMatch { is_swap_source: Some(false), ..Default::default() }],
        WindowRuleCandidate { is_swap_source: true, ..Default::default() } => false;
        "is_swap_source_mismatch"
    )]
    #[test_case(
        vec![WindowRuleMatch { is_swap_target: Some(true), ..Default::default() }],
        WindowRuleCandidate { is_swap_target: true, ..Default::default() } => true;
        "is_swap_target_match"
    )]
    #[test_case(
        vec![WindowRuleMatch { is_swap_target: Some(false), ..Default::default() }],
        WindowRuleCandidate { is_swap_target: true, ..Default::default() } => false;
        "is_swap_target_mismatch"
    )]
    #[test_case(
        vec![WindowRuleMatch { workspace_name: Some(WorkspaceName::Id(1)), ..Default::default() }],
        WindowRuleCandidate { workspace_name: WorkspaceName::Id(1), ..Default::default() } => true;
        "workspace_name_match"
    )]
    #[test_case(
        vec![WindowRuleMatch { workspace_name: Some(WorkspaceName::Id(1)), ..Default::default() }],
        WindowRuleCandidate { workspace_name: WorkspaceName::Id(2), ..Default::default() } => false;
        "workspace_name_mismatch"
    )]
    fn test_match_window_rule(
        matches: Vec<WindowRuleMatch>,
        candidate: WindowRuleCandidate,
    ) -> bool {
        (WindowRule {
            matches,
            properties: WindowProperties::default(),
        })
        .is_match(&candidate)
    }

    #[test_case(
        WindowProperties { opening: None, dynamic: WindowDynamicProperties::default() },
        WindowProperties { opening: Some(WindowOpeningProperties::default()), dynamic: WindowDynamicProperties::default() } =>
        WindowProperties { opening: None, dynamic: WindowDynamicProperties::default() };
        "lhs_none_rhs_some_keeps_none"
    )]
    #[test_case(
        WindowProperties { opening: Some(WindowOpeningProperties::default()), dynamic: WindowDynamicProperties::default() },
        WindowProperties { opening: None, dynamic: WindowDynamicProperties::default() } =>
        WindowProperties { opening: Some(WindowOpeningProperties::default()), dynamic: WindowDynamicProperties::default() };
        "lhs_some_rhs_none_keeps_some"
    )]
    #[test_case(
        WindowProperties {
            opening: Some(WindowOpeningProperties { focus: Some(true), state: None, workspace_name: None }),
            dynamic: WindowDynamicProperties::default()
        },
        WindowProperties {
            opening: Some(WindowOpeningProperties { focus: Some(false), state: None, workspace_name: None }),
            dynamic: WindowDynamicProperties::default()
        } =>
        WindowProperties {
            opening: Some(WindowOpeningProperties { focus: Some(false), state: None, workspace_name: None }),
            dynamic: WindowDynamicProperties::default()
        };
        "both_some_rhs_opening_overrides"
    )]
    #[test_case(
        WindowProperties {
            opening: Some(WindowOpeningProperties::default()),
            dynamic: WindowDynamicProperties { decoration: Some(WindowDecoration::ClientSide), border: None, opacity: None }
        },
        WindowProperties {
            opening: Some(WindowOpeningProperties::default()),
            dynamic: WindowDynamicProperties { decoration: Some(WindowDecoration::ServerSide), border: None, opacity: None }
        } =>
        WindowProperties {
            opening: Some(WindowOpeningProperties::default()),
            dynamic: WindowDynamicProperties { decoration: Some(WindowDecoration::ServerSide), border: None, opacity: None }
        };
        "both_some_rhs_dynamic_overrides"
    )]
    fn test_merge_properties(lhs: WindowProperties, rhs: WindowProperties) -> WindowProperties {
        lhs.merge(rhs)
    }
}
