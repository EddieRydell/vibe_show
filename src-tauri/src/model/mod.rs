pub mod color;
pub mod color_gradient;
pub mod curve;
pub mod fixture;
pub mod show;
pub mod timeline;

// Re-export commonly used types at the model level.
pub use color::Color;
pub use color_gradient::{ColorGradient, ColorStop};
pub use curve::{Curve, CurvePoint};
pub use fixture::{
    BulbShape, ChannelOrder, Controller, ControllerId, EffectTarget, FixtureDef, FixtureGroup,
    FixtureId, GroupId, GroupMember, OutputMapping, Patch, PixelType,
};
pub use show::{Layout, LayoutShape, Show};
pub use timeline::{
    BlendMode, ColorMode, EffectInstance, EffectKind, EffectParams, ParamKey, ParamSchema,
    ParamType, ParamValue, Sequence, TimeRange, Track,
};
