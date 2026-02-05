use regex::Regex;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;

pub struct WindowRules(Vec<WindowRule>);

impl WindowRules {
    pub fn get_config(&self, candidate: &WindowRuleMatch) -> WindowProperties {
        self.0
            .iter()
            .find(|r| r.is_match(candidate))
            .map(|r| r.properties.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug)]
struct WindowRule {
    matches: Vec<WindowRuleMatch>,
    properties: WindowProperties,
}

impl WindowRule {
    fn is_match(&self, candidate: &WindowRuleMatch) -> bool {
        self.matches.iter().any(|target| {
            match (&target.app_id, &candidate.app_id) {
                (Some(re), Some(hay)) => {
                    let re = Regex::new(re).unwrap();
                    if !re.is_match(hay) {
                        return false;
                    }
                }
                (Some(_), None) => return false,
                (None, _) => {}
            }
            match (&target.title, &candidate.title) {
                (Some(re), Some(hay)) => {
                    let re = Regex::new(re).unwrap();
                    if !re.is_match(hay) {
                        return false;
                    }
                }
                (Some(_), None) => return false,
                (None, _) => {}
            }
            if target.focus.is_some() && target.focus != candidate.focus {
                return false;
            }
            if target.float.is_some() && target.float != candidate.float {
                return false;
            }
            if target.workspace.is_some() && target.workspace != candidate.workspace {
                return false;
            }

            true
        })
    }
}

#[derive(Debug, Clone)]
pub struct WindowProperties {
    pub decoration: WindowDecoration,
    pub focus: bool,
    pub float: Option<WindowFloat>,
    pub workspace: Option<u8>,
}

impl Default for WindowProperties {
    fn default() -> Self {
        Self {
            decoration: WindowDecoration::default(),
            focus: true,
            float: None,
            workspace: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct WindowRuleMatch {
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub focus: Option<bool>,
    pub float: Option<bool>,
    pub workspace: Option<u8>,
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
    WindowRules(vec![WindowRule {
        matches: vec![WindowRuleMatch {
            // app_id: Some("Alacritty".to_string()),
            ..Default::default()
        }],
        properties: WindowProperties {
            focus: false,
            ..Default::default()
        },
    }])
}
