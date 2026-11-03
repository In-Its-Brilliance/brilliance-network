use strum_macros::EnumIter;

/// Animation clip names used by character models.
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
