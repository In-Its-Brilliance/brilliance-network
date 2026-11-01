use std::collections::HashMap;

use common::inventory::item::BodyPart;
use entity_tag::EntityTagData;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum AnimationClip {
    Stand,
    Walk,
    Run,
    WalkBackwards,
    ShuffleLeft,
    ShuffleRight,
    JumpStart,
    Jump,
    Fall,
    JumpEnd,
}

impl AnimationClip {
    pub fn as_name(self) -> &'static str {
        match self {
            Self::Stand => "stand",
            Self::Walk => "walk",
            Self::Run => "run",
            Self::WalkBackwards => "walkbackwards",
            Self::ShuffleLeft => "shuffle_left",
            Self::ShuffleRight => "shuffle_right",
            Self::JumpStart => "jump_start",
            Self::Jump => "jump",
            Self::Fall => "fall",
            Self::JumpEnd => "jump_end",
        }
    }

    pub fn loops(self) -> bool {
        !matches!(self, Self::JumpStart | Self::JumpEnd)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum EntitySkinData {
    Generic { body_parts: HashMap<BodyPart, String> },
    Fixed(String),
    None,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum EntityNetworkComponent {
    Tag(Option<EntityTagData>),
    Skin(EntitySkinData),
}
