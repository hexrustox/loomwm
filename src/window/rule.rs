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

#[derive(Debug, Default, Deserialize, Hash)]
#[serde(rename_all = "kebab-case")]
pub struct WindowRuleMatch {
    app_id: Option<String>,
    title: Option<String>,
    focus: Option<bool>,
    float: Option<bool>,
    workspace: Option<WorkspaceName>,
}

#[derive(Debug, Clone, Deserialize, Hash)]
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

#[derive(Debug, Default, Clone, Deserialize, Hash)]
pub struct WindowOpeningProperties {
    pub focus: Option<bool>,
    #[serde(flatten)]
    pub state: Option<WindowState>,
    pub workspace: Option<WorkspaceName>,
}

#[derive(Debug, Default, Clone, Deserialize)]
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
    fn merge(self, rhs: Self) -> Self {
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
            workspace: rhs.workspace.or(self.workspace),
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
    pub workspace: WorkspaceName,
}

#[derive(Debug, Clone, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
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

#[derive(Debug, Clone, Deserialize, Hash)]
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

#[derive(Debug, Clone, Deserialize, Hash)]
#[serde(untagged)]
#[serde(rename_all = "lowercase")]
pub enum WindowLocation {
    Center,
    Location(N, N),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct T {
        #[allow(dead_code)]
        x: WindowRules,
    }

    #[test]
    fn test_deserialize() {
        toml::from_str::<T>(
            r#"[[x]]
matches = [{ app-id = "test", title = "test" }]
focus = false
float = {}

[[x]]
matches = [{ float = true }]
tile = 1.5
"#,
        )
        .unwrap();
    }
}
