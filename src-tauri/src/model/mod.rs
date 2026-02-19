pub mod color;
pub mod fixture;
pub mod show;
pub mod timeline;

// Re-export commonly used types at the model level.
pub use color::Color;
pub use fixture::{
    ChannelOrder, Controller, ControllerId, EffectTarget, FixtureDef, FixtureGroup, FixtureId,
    GroupId, GroupMember, OutputMapping, Patch,
};
pub use show::{Layout, Show};
pub use timeline::{
    BlendMode, EffectInstance, EffectKind, EffectParams, ParamSchema, ParamType, ParamValue,
    Sequence, TimeRange, Track,
};
