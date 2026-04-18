mod inspect;
mod subset;
mod variation;

pub use inspect::{inspect_otf_font, CffError, CffFontKind};
pub use subset::{
    serialize_subset_otf, subset_static_cff, subset_variable_cff2, OtfSubsetResult,
};
pub use variation::{parse_variation_axes, VariationAxisValue};
