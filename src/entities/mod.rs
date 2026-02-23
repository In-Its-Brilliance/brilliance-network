use entity_tag::EntityTagData;
use serde::{Deserialize, Serialize};

pub mod entity_tag;

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum EntitySkinData {
    Generic,
    Fixed(String),
    None,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum EntityNetworkComponent {
    Tag(Option<EntityTagData>),
    Skin(EntitySkinData),
}
