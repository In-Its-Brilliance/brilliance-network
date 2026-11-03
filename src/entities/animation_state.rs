use serde::{Deserialize, Serialize};

/// High-level character animation state synchronized over the network.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum AnimationState {
    Idle,
    Walk,
    Run,
    Jump,
    Fall,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self::Idle
    }
}

impl AnimationState {
    pub fn from_name(name: &str) -> Self {
        match name {
            "idle" => Self::Idle,
            "walk" => Self::Walk,
            "run" => Self::Run,
            "jump" => Self::Jump,
            "fall" => Self::Fall,
            _ => Self::Idle,
        }
    }

    pub fn as_name(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Walk => "walk",
            Self::Run => "run",
            Self::Jump => "jump",
            Self::Fall => "fall",
        }
    }
}
