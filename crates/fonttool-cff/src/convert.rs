use std::ffi::{c_char, c_void, CString};

use allsorts::{
    binary::read::ReadScope,
    cff::{outline::CFFOutlines, CFF},
    font::{Font, GlyphTableFlags},
    font_data::FontData,
    outline::{OutlineBuilder, OutlineSink},
    pathfinder_geometry::{line_segment::LineSegment2F, vector::Vector2F},
    tables::{FontTableProvider, SfntVersion},
    tag,
};

use crate::{
    inspect_otf_font, instantiate_variable_cff2, load_font_source, variation::VariationAxisValue,
    CffError,
};

const EOT_OK: i32 = 0;
const CU2QU_MAX_ERROR: f64 = 1.0;

#[derive(Debug, Clone)]
struct OutlinePoint {
    x: i16,
    y: i16,
    on_curve: bool,
}

#[derive(Debug, Clone)]
struct OutlineRecord {
    contours: Vec<Vec<OutlinePoint>>,
    advance_width: u16,
    glyph_name: Option<String>,
}

#[repr(C)]
struct NativePoint {
    x: i16,
    y: i16,
    on_curve: i32,
}

#[repr(C)]
struct NativeContour {
    points: *mut NativePoint,
    num_points: usize,
}

#[repr(C)]
struct NativeGlyphOutline {
    contours: *mut NativeContour,
    num_contours: usize,
    advance_width: u16,
    glyph_name: *mut c_char,
}

#[repr(C)]
struct NativeSfntTable {
    tag: u32,
    data: *mut u8,
    length: usize,
}

#[repr(C)]
struct NativeSfntFont {
    tables: *mut NativeSfntTable,
    num_tables: usize,
    capacity: usize,
}

#[repr(C)]
struct NativeCurvePoint {
    x: f64,
    y: f64,
}

#[repr(C)]
struct NativeCubicCurve {
    p0: NativeCurvePoint,
    p1: NativeCurvePoint,
    p2: NativeCurvePoint,
    p3: NativeCurvePoint,
}

#[repr(C)]
struct NativeQuadraticSpline {
    points: *mut NativeCurvePoint,
    num_points: usize,
    closed: i32,
}

unsafe extern "C" {
    fn tt_rebuilder_build_font(
        outlines: *const NativeGlyphOutline,
        num_outlines: usize,
        out_font: *mut NativeSfntFont,
    ) -> i32;

    fn sfnt_writer_serialize(
        font: *mut NativeSfntFont,
        out_data: *mut *mut u8,
        out_size: *mut usize,
    ) -> i32;

    fn sfnt_font_init(font: *mut NativeSfntFont);
    fn sfnt_font_destroy(font: *mut NativeSfntFont);
    fn curve_to_quadratic(
        curve: *const NativeCubicCurve,
        max_err: f64,
        out_spline: *mut NativeQuadraticSpline,
    ) -> i32;
    fn quadratic_spline_destroy(spline: *mut NativeQuadraticSpline);
    fn free(ptr: *mut c_void);
}

pub fn convert_otf_to_ttf(
    bytes: &[u8],
    axes: &[VariationAxisValue],
) -> Result<Vec<u8>, CffError> {
    let source_bytes = load_font_source(bytes)?;
    let kind = inspect_otf_font(&source_bytes)?;
    if !kind.is_cff_flavor {
        return Err(CffError::InvalidInput(
            "convert expects OTF/CFF or OTF/CFF2 input".to_string(),
        ));
    }

    let static_source = if kind.is_variable {
        instantiate_variable_cff2(&source_bytes, axes)?
    } else {
        if !axes.is_empty() {
            return Err(CffError::VariationRejectedForStaticInput);
        }
        source_bytes
    };

    let outlines = collect_allsorts_outlines(&static_source)?;
    rebuild_truetype_from_outlines(&outlines)
}

fn rebuild_truetype_from_outlines(outlines: &[OutlineRecord]) -> Result<Vec<u8>, CffError> {
    let mut point_storage = Vec::with_capacity(outlines.len());
    let mut contour_storage = Vec::with_capacity(outlines.len());
    let mut name_storage = Vec::with_capacity(outlines.len());
    let mut native_outlines = Vec::with_capacity(outlines.len());

    for outline in outlines {
        let mut native_points_by_contour = outline
            .contours
            .iter()
            .map(|contour| contour.iter().map(point_to_native).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let mut native_contours = native_points_by_contour
            .iter_mut()
            .map(|points| NativeContour {
                points: points.as_mut_ptr(),
                num_points: points.len(),
            })
            .collect::<Vec<_>>();
        let native_name = outline
            .glyph_name
            .as_deref()
            .map(sanitize_c_string)
            .transpose()
            .map_err(|error| CffError::EncodeFailed(error.to_string()))?;
        let glyph_name = native_name
            .as_ref()
            .map_or(std::ptr::null_mut(), |value| value.as_ptr().cast_mut());

        native_outlines.push(NativeGlyphOutline {
            contours: native_contours.as_mut_ptr(),
            num_contours: native_contours.len(),
            advance_width: outline.advance_width,
            glyph_name,
        });
        point_storage.push(native_points_by_contour);
        contour_storage.push(native_contours);
        name_storage.push(native_name);
    }

    let mut font = NativeSfntFont {
        tables: std::ptr::null_mut(),
        num_tables: 0,
        capacity: 0,
    };
    unsafe { sfnt_font_init(&mut font) };

    let mut serialized_ptr = std::ptr::null_mut();
    let mut serialized_len = 0usize;
    let rebuild_status =
        unsafe { tt_rebuilder_build_font(native_outlines.as_ptr(), native_outlines.len(), &mut font) };
    if rebuild_status != 0 {
        unsafe { sfnt_font_destroy(&mut font) };
        return Err(CffError::EncodeFailed(format!(
            "TrueType rebuild failed with status {rebuild_status}"
        )));
    }

    let serialize_status =
        unsafe { sfnt_writer_serialize(&mut font, &mut serialized_ptr, &mut serialized_len) };
    unsafe { sfnt_font_destroy(&mut font) };
    if serialize_status != 0 {
        return Err(CffError::EncodeFailed(format!(
            "TrueType serialization failed with status {serialize_status}"
        )));
    }
    if serialized_ptr.is_null() {
        return Err(CffError::EncodeFailed(
            "TrueType serialization returned no output".to_string(),
        ));
    }

    let output = unsafe {
        let slice = std::slice::from_raw_parts(serialized_ptr, serialized_len);
        let bytes = slice.to_vec();
        free(serialized_ptr.cast::<c_void>());
        bytes
    };
    Ok(output)
}

fn sanitize_c_string(name: &str) -> Result<CString, std::ffi::NulError> {
    CString::new(name.replace('\0', ""))
}

fn point_to_native(point: &OutlinePoint) -> NativePoint {
    NativePoint {
        x: point.x,
        y: point.y,
        on_curve: if point.on_curve { 1 } else { 0 },
    }
}

fn collect_allsorts_outlines(bytes: &[u8]) -> Result<Vec<OutlineRecord>, CffError> {
    let scope = ReadScope::new(bytes);
    let font_file = scope
        .read::<FontData<'_>>()
        .map_err(|error| CffError::InvalidInput(format!("invalid OTF source: {error}")))?;
    let provider = font_file
        .table_provider(0)
        .map_err(|error| CffError::InvalidInput(format!("invalid OTF source: {error}")))?;
    let mut font =
        Font::new(provider).map_err(|error| CffError::InvalidInput(format!("invalid OTF source: {error}")))?;

    if !font.glyph_table_flags.contains(GlyphTableFlags::CFF)
        || font.font_table_provider.sfnt_version() != tag::OTTO
    {
        return Err(CffError::EncodeFailed(
            "allsorts outline extraction currently expects a static CFF font".to_string(),
        ));
    }

    let glyph_ids = (0..font.num_glyphs()).collect::<Vec<_>>();
    let advance_widths = glyph_ids
        .iter()
        .map(|&glyph_id| font.horizontal_advance(glyph_id).unwrap_or(0))
        .collect::<Vec<_>>();
    let glyph_names = font
        .glyph_names(&glyph_ids)
        .into_iter()
        .map(|name| name.into_owned())
        .collect::<Vec<_>>();

    let cff_data = font
        .font_table_provider
        .read_table_data(tag::CFF)
        .map_err(|error| CffError::EncodeFailed(format!("failed to read CFF table: {error}")))?
        .into_owned();
    let cff = ReadScope::new(&cff_data)
        .read::<CFF<'_>>()
        .map_err(|error| CffError::EncodeFailed(format!("failed to parse CFF table: {error}")))?;
    let mut builder = CFFOutlines { table: &cff };
    let mut outlines = Vec::with_capacity(glyph_ids.len());

    for (index, glyph_id) in glyph_ids.iter().copied().enumerate() {
        let advance_width = advance_widths[index];
        let glyph_name = glyph_names.get(glyph_id as usize).cloned();
        let mut sink = QuadraticOutlineSink::default();
        builder
            .visit(glyph_id, None, &mut sink)
            .map_err(|error| CffError::EncodeFailed(format!("failed to visit glyph outline: {error}")))?;
        let contours = sink.finish()?;
        outlines.push(OutlineRecord {
            contours,
            advance_width,
            glyph_name,
        });
    }

    Ok(outlines)
}

#[derive(Default)]
struct QuadraticOutlineSink {
    contours: Vec<Vec<OutlinePoint>>,
    current_contour: Vec<OutlinePoint>,
    current_x: f32,
    current_y: f32,
    error: Option<CffError>,
}

impl QuadraticOutlineSink {
    fn finish(mut self) -> Result<Vec<Vec<OutlinePoint>>, CffError> {
        if let Some(error) = self.error.take() {
            return Err(error);
        }
        if !self.current_contour.is_empty() {
            self.contours.push(std::mem::take(&mut self.current_contour));
        }
        Ok(self.contours)
    }

    fn remember(&mut self, result: Result<(), CffError>) {
        if self.error.is_none() {
            if let Err(error) = result {
                self.error = Some(error);
            }
        }
    }

    fn append_point(&mut self, x: f32, y: f32, on_curve: bool) -> Result<(), CffError> {
        let point = OutlinePoint {
            x: round_to_i16(x)?,
            y: round_to_i16(y)?,
            on_curve,
        };

        if self
            .current_contour
            .last()
            .is_some_and(|last| last.x == point.x && last.y == point.y && last.on_curve == point.on_curve)
        {
            return Ok(());
        }

        self.current_contour.push(point);
        Ok(())
    }

    fn finish_current_contour(&mut self) {
        if !self.current_contour.is_empty() {
            self.contours.push(std::mem::take(&mut self.current_contour));
        }
    }
}

impl OutlineSink for QuadraticOutlineSink {
    fn move_to(&mut self, to: Vector2F) {
        if self.error.is_some() {
            return;
        }
        self.finish_current_contour();
        let result = self.append_point(to.x(), to.y(), true);
        self.remember(result);
        self.current_x = to.x();
        self.current_y = to.y();
    }

    fn line_to(&mut self, to: Vector2F) {
        if self.error.is_some() {
            return;
        }
        let result = self.append_point(to.x(), to.y(), true);
        self.remember(result);
        self.current_x = to.x();
        self.current_y = to.y();
    }

    fn quadratic_curve_to(&mut self, ctrl: Vector2F, to: Vector2F) {
        if self.error.is_some() {
            return;
        }
        let control_result = self.append_point(ctrl.x(), ctrl.y(), false);
        self.remember(control_result);
        let target_result = self.append_point(to.x(), to.y(), true);
        self.remember(target_result);
        self.current_x = to.x();
        self.current_y = to.y();
    }

    fn cubic_curve_to(&mut self, ctrl: LineSegment2F, to: Vector2F) {
        if self.error.is_some() {
            return;
        }

        let cubic = NativeCubicCurve {
            p0: NativeCurvePoint {
                x: self.current_x as f64,
                y: self.current_y as f64,
            },
            p1: NativeCurvePoint {
                x: ctrl.from_x() as f64,
                y: ctrl.from_y() as f64,
            },
            p2: NativeCurvePoint {
                x: ctrl.to_x() as f64,
                y: ctrl.to_y() as f64,
            },
            p3: NativeCurvePoint {
                x: to.x() as f64,
                y: to.y() as f64,
            },
        };
        let mut spline = NativeQuadraticSpline {
            points: std::ptr::null_mut(),
            num_points: 0,
            closed: 0,
        };
        let status = unsafe { curve_to_quadratic(&cubic, CU2QU_MAX_ERROR, &mut spline) };
        if status != EOT_OK {
            self.error = Some(CffError::EncodeFailed(format!(
                "failed to convert cubic outline segment to quadratic spline: status {status}"
            )));
            return;
        }

        let result = unsafe {
            let points = std::slice::from_raw_parts(spline.points, spline.num_points);
            let mut append_result = Ok(());
            for (index, point) in points.iter().enumerate().skip(1) {
                let on_curve = index + 1 == points.len();
                if let Err(error) = self.append_point(point.x as f32, point.y as f32, on_curve) {
                    append_result = Err(error);
                    break;
                }
            }
            quadratic_spline_destroy(&mut spline);
            append_result
        };
        self.remember(result);
        self.current_x = to.x();
        self.current_y = to.y();
    }

    fn close(&mut self) {
        if self.error.is_some() {
            return;
        }
        self.finish_current_contour();
    }
}

fn round_to_i16(value: f32) -> Result<i16, CffError> {
    if !value.is_finite() {
        return Err(CffError::EncodeFailed(
            "outline coordinate is not finite".to_string(),
        ));
    }

    let rounded = value.round();
    if rounded < i16::MIN as f32 || rounded > i16::MAX as f32 {
        return Err(CffError::EncodeFailed(
            "outline coordinate is outside int16 range".to_string(),
        ));
    }

    Ok(rounded as i16)
}
