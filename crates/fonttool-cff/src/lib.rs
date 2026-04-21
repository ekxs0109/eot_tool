mod convert;
mod inspect;
mod instance;
mod office_static;
mod source;
mod subset;
mod variation;

pub use convert::convert_otf_to_ttf;
pub use inspect::{inspect_otf_font, CffError, CffFontKind};
pub use instance::instantiate_variable_cff2;
pub use office_static::{
    extract_office_static_cff, rebuild_office_static_cff_sfnt, rebuild_office_static_cff_table,
    OfficeStaticCff,
};
pub use source::load_font_source;
pub use subset::{serialize_subset_otf, subset_static_cff, subset_variable_cff2, OtfSubsetResult};
pub use variation::{parse_variation_axes, VariationAxisValue};
