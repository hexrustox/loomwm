use regex::Regex;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;

use crate::monitor::WorkspaceName;

#[derive(Debug)]
pub struct WindowRules(Vec<WindowRule>);

impl WindowRules {
    pub fn get_properties(&self, mut candidate: WindowRuleCandidate) -> WindowProperties {
        let mut properties = WindowProperties::default();
        for rule in &self.0 {
            if rule.is_match(&candidate) {
                properties = properties.merge(rule.properties.clone());
                if let Some(focus) = properties.focus {
                    candidate.focus = focus;
                }
                if properties.float.is_some() {
                    candidate.float = true;
                }
                if let Some(workspace) = properties.workspace.as_ref() {
                    candidate.workspace = workspace.clone();
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

#[derive(Debug, Default, Clone)]
pub struct WindowProperties {
    pub decoration: Option<WindowDecoration>,
    pub focus: Option<bool>,
    pub float: Option<WindowFloat>,
    pub workspace: Option<WorkspaceName>,
}

impl WindowProperties {
    pub fn merge(self, rhs: Self) -> Self {
        Self {
            decoration: rhs.decoration.or(self.decoration),
            focus: rhs.focus.or(self.focus),
            float: rhs.float.or(self.float),
            workspace: rhs.workspace.or(self.workspace),
        }
    }
}

pub struct WindowRuleCandidate {
    pub app_id: String,
    pub title: String,
    pub focus: bool,
    pub float: bool,
    pub workspace: WorkspaceName,
}

#[derive(Debug, Default, Clone)]
pub enum WindowDecoration {
    ClientSide,
    #[default]
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
pub struct WindowFloat {
    pub location: Option<WindowLocation>,
    pub size: Option<(N, N)>,
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
                app_id: Some("Alacritty".to_string()),
                ..Default::default()
            }],
            properties: WindowProperties {
                float: Some(WindowFloat {
                    location: Some(WindowLocation::Center),
                    size: None,
                }),
                ..Default::default()
            },
        },
        WindowRule {
            matches: vec![WindowRuleMatch {
                float: Some(true),
                ..Default::default()
            }],
            properties: WindowProperties {
                decoration: Some(WindowDecoration::ClientSide),
                ..Default::default()
            },
        },
    ])
}
