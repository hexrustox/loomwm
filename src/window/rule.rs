use regex::Regex;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;

use crate::monitor::{TileRatio, WorkspaceName};

#[derive(Debug)]
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
                    workspace: Some(ref name),
                    ..
                }) = properties.opening
                {
                    candidate.workspace = name.clone();
                }
            }
        }

        properties
    }
}

#[derive(Debug)]
struct WindowRule {
    matches: Vec<WindowRuleMatch>,
    properties: WindowProperties,
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
                .workspace
                .as_ref()
                .is_some_and(|workspace| candidate.workspace != *workspace)
            {
                return false;
            }

            true
        })
    }
}

#[derive(Debug, Default)]
pub struct WindowRuleMatch {
    app_id: Option<String>,
    title: Option<String>,
    focus: Option<bool>,
    float: Option<bool>,
    workspace: Option<WorkspaceName>,
}

#[derive(Debug, Clone)]
pub struct WindowProperties {
    pub opening: Option<WindowOpeningProperties>,
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

#[derive(Debug, Default, Clone)]
pub struct WindowOpeningProperties {
    pub focus: Option<bool>,
    pub state: Option<WindowState>,
    pub workspace: Option<WorkspaceName>,
}

#[derive(Debug, Default, Clone)]
pub struct WindowDynamicProperties {
    pub decoration: Option<WindowDecoration>,
}

impl WindowProperties {
    fn merge(self, rhs: Self) -> Self {
        Self {
            opening: self
                .opening
                .zip(rhs.opening)
                .map(|(this, rhs)| this.merge(rhs)),
            dynamic: self.dynamic.merge(rhs.dynamic),
        }
    }
}

impl WindowOpeningProperties {
    fn merge(self, rhs: Self) -> Self {
        Self {
            focus: rhs.focus.or(self.focus),
            state: rhs.state.or(self.state),
            workspace: rhs.workspace.or(self.workspace),
        }
    }
}

impl WindowDynamicProperties {
    fn merge(self, rhs: Self) -> Self {
        Self {
            decoration: rhs.decoration.or(self.decoration),
        }
    }
}

#[derive(Debug)]
pub struct WindowRuleCandidate {
    pub app_id: String,
    pub title: String,
    pub focus: bool,
    pub float: bool,
    pub workspace: WorkspaceName,
}

#[derive(Debug, Clone)]
pub enum WindowDecoration {
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

#[derive(Debug, Clone)]
pub enum WindowState {
    Float {
        location: Option<WindowLocation>,
        size: Option<(N, N)>,
    },
    Tile(Option<TileRatio>),
}

impl Default for WindowState {
    fn default() -> Self {
        Self::Tile(None)
    }
}

#[derive(Debug, Clone)]
pub enum WindowLocation {
    Center,
    Location(N, N),
}

// TEMP
pub fn test_window_rules() -> WindowRules {
    WindowRules(vec![
        WindowRule {
            matches: vec![WindowRuleMatch {
                ..Default::default()
            }],
            properties: WindowProperties {
                opening: Some(WindowOpeningProperties {
                    focus: Some(false),
                    ..Default::default()
                }),
                dynamic: WindowDynamicProperties {
                    decoration: Some(WindowDecoration::ServerSide),
                },
            },
        },
        WindowRule {
            matches: vec![WindowRuleMatch {
                float: Some(true),
                ..Default::default()
            }],
            properties: WindowProperties {
                opening: None,
                dynamic: WindowDynamicProperties {
                    decoration: Some(WindowDecoration::ClientSide),
                },
            },
        },
    ])
}
