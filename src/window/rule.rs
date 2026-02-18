use std::hash::{DefaultHasher, Hash, Hasher};

use regex::Regex;
use serde::Deserialize;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;

use crate::monitor::{TileRatio, WorkspaceName};

#[derive(Debug, Default, Deserialize, Hash)]
#[serde(transparent)]
pub struct WindowRules(Vec<WindowRule>);

impl WindowRules {
    pub fn get_properties(
        &self,
        mut candidate: WindowRuleCandidate,
        opening: bool,
    ) -> WindowProperties {
        let mut properties = if opening {
            WindowProperties::default()
        } else {
            WindowProperties {
                opening: None,
                ..Default::default()
            }
        };
        for rule in &self.0 {
            if rule.is_match(&candidate) {
                properties = properties.merge(rule.properties.clone());

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

    pub fn get_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

#[derive(Debug, Deserialize, Hash)]
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
        self.matches.iter().any(|target| {
            if let Some(app_id_pattern) = &target.app_id
                && let Ok(re) = Regex::new(app_id_pattern)
                && !re.is_match(&candidate.app_id)
            {
                return false;
            };
            if let Some(title_pattern) = &target.title
                && let Ok(re) = Regex::new(title_pattern)
                && !re.is_match(&candidate.title)
            {
                return false;
            };
            if target.focus.is_some_and(|focus| candidate.focus != focus) {
                return false;
            }
            if target.float.is_some_and(|float| candidate.float != float) {
                return false;
            }
            if target
                .workspace_name
                .as_ref()
                .is_some_and(|workspace_name| candidate.workspace_name != *workspace_name)
            {
                return false;
            }

            true
        })
    }
}

// TODO tags
#[derive(Debug, Default, Deserialize, Hash)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct WindowRuleMatch {
    app_id: Option<String>,
    title: Option<String>,
    #[serde(rename = "is-focused")]
    focus: Option<bool>,
    #[serde(rename = "is-floating")]
    float: Option<bool>,
    #[serde(rename = "in-workspace")]
    workspace_name: Option<WorkspaceName>,
}

#[derive(Debug, Clone, Deserialize, Hash, PartialEq)]
pub struct WindowProperties {
    #[serde(flatten)]
    pub opening: Option<WindowOpeningProperties>,
    #[serde(flatten)]
    #[serde(default)]
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

#[derive(Debug, Default, Clone, Deserialize, Hash, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WindowOpeningProperties {
    #[serde(rename = "open-with-focus")]
    pub focus: Option<bool>,
    #[serde(flatten)]
    pub state: Option<WindowState>,
    #[serde(rename = "open-in-workspace")]
    pub workspace_name: Option<WorkspaceName>,
}

// TODO border
#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WindowDynamicProperties {
    pub decoration: Option<WindowDecoration>,
    pub opacity: Option<f32>,
}

impl std::hash::Hash for WindowDynamicProperties {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.decoration.hash(state);
        if let Some(f) = self.opacity {
            f.to_bits().hash(state)
        }
    }
}

impl WindowProperties {
    pub fn merge(self, rhs: Self) -> Self {
        Self {
            opening: if let Some(rhs) = rhs.opening {
                self.opening.map(|opening| opening.override_with(rhs))
            } else {
                self.opening
            },
            dynamic: self.dynamic.override_with(rhs.dynamic),
        }
    }
}

impl WindowOpeningProperties {
    fn override_with(self, rhs: Self) -> Self {
        Self {
            focus: rhs.focus.or(self.focus),
            state: rhs.state.or(self.state),
            workspace_name: rhs.workspace_name.or(self.workspace_name),
        }
    }
}

impl WindowDynamicProperties {
    fn override_with(self, rhs: Self) -> Self {
        Self {
            decoration: rhs.decoration.or(self.decoration),
            opacity: rhs.opacity.or(self.opacity),
        }
    }
}

#[derive(Debug)]
pub struct WindowRuleCandidate {
    pub app_id: String,
    pub title: String,
    pub focus: bool,
    pub float: bool,
    pub workspace_name: WorkspaceName,
}

#[derive(Debug, Default, Clone, Deserialize, Hash, PartialEq)]
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

impl std::hash::Hash for WindowState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Float { location, size } => {
                location.hash(state);
                size.hash(state);
            }
            Self::Tile { ratio } => {
                if let Some(r) = ratio {
                    r.to_bits().hash(state);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq)]
pub enum WindowLocation {
    Center,
    Location(N, N),
}
