use std::borrow::Borrow;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::Range;

use super::dual_coord::DualCoordChartContext;
use super::mesh::MeshStyle;
use super::series::SeriesLabelStyle;

use crate::coord::{
    AsRangedCoord, CoordTranslate, MeshLine, Ranged, RangedCoord, ReverseCoordTranslate, Shift,
};
use crate::drawing::backend::{BackendCoord, DrawingBackend};
use crate::drawing::{DrawingArea, DrawingAreaErrorKind};
use crate::element::{Drawable, DynElement, IntoDynElement, Path, PointCollection};
use crate::style::{FontTransform, ShapeStyle, TextStyle};

/// The annotations (such as the label of the series, the legend element, etc)
pub struct SeriesAnno<'a, DB: DrawingBackend> {
    label: Option<String>,
    draw_func: Option<Box<dyn Fn(BackendCoord) -> DynElement<'a, DB, BackendCoord> + 'a>>,
    phantom_data: PhantomData<DB>,
}

impl<'a, DB: DrawingBackend> SeriesAnno<'a, DB> {
    pub(crate) fn get_label(&self) -> &str {
        self.label.as_ref().map(|x| x.as_str()).unwrap_or("")
    }

    pub(crate) fn get_draw_func(
        &self,
    ) -> Option<&dyn Fn(BackendCoord) -> DynElement<'a, DB, BackendCoord>> {
        self.draw_func.as_ref().map(|x| x.borrow())
    }

    fn new() -> Self {
        Self {
            label: None,
            draw_func: None,
            phantom_data: PhantomData,
        }
    }

    /// Set the series label
    /// `label`: The string would be use as label for current series
    pub fn label<L: Into<String>>(&mut self, label: L) -> &mut Self {
        self.label = Some(label.into());
        self
    }

    /// Set the legend element creator function
    /// - `func`: The function use to create the element
    /// *Note*: The creation function uses a shifted pixel-based coordinate system. And place the
    /// point (0,0) to the mid-right point of the shape
    pub fn legend<E: IntoDynElement<'a, DB, BackendCoord>, T: Fn(BackendCoord) -> E + 'a>(
        &mut self,
        func: T,
    ) -> &mut Self {
        self.draw_func = Some(Box::new(move |p| func(p).into_dyn()));
        self
    }
}

/// The context of the chart. This is the core object of Plotters.
/// Any plot/chart is abstracted as this type, and any data series can be placed to the chart
/// context.
pub struct ChartContext<'a, DB: DrawingBackend, CT: CoordTranslate> {
    pub(super) x_label_area: [Option<DrawingArea<DB, Shift>>; 2],
    pub(super) y_label_area: [Option<DrawingArea<DB, Shift>>; 2],
    pub(super) drawing_area: DrawingArea<DB, CT>,
    pub(super) series_anno: Vec<SeriesAnno<'a, DB>>,
}

impl<
        'a,
        DB: DrawingBackend,
        XT: Debug,
        YT: Debug,
        X: Ranged<ValueType = XT>,
        Y: Ranged<ValueType = YT>,
    > ChartContext<'a, DB, RangedCoord<X, Y>>
{
    /// Initialize a mesh configuration object and mesh drawing can be finalized by calling
    /// the function `MeshStyle::draw`
    pub fn configure_mesh<'b>(&'b mut self) -> MeshStyle<'a, 'b, X, Y, DB> {
        MeshStyle {
            axis_style: None,
            x_label_offset: 0,
            y_label_offset: 0,
            draw_x_mesh: true,
            draw_y_mesh: true,
            draw_x_axis: true,
            draw_y_axis: true,
            n_x_labels: 10,
            n_y_labels: 10,
            line_style_1: None,
            line_style_2: None,
            label_style: None,
            format_x: &|x| format!("{:?}", x),
            format_y: &|y| format!("{:?}", y),
            target: Some(self),
            _pahtom_data: PhantomData,
            x_desc: None,
            y_desc: None,
            axis_desc_style: None,
        }
    }
}

impl<'a, DB: DrawingBackend + 'a, CT: CoordTranslate> ChartContext<'a, DB, CT> {
    /// Configure the styles for drawing series labels in the chart
    pub fn configure_series_labels<'b>(&'b mut self) -> SeriesLabelStyle<'a, 'b, DB, CT> {
        SeriesLabelStyle::new(self)
    }

    /// Get a reference of underlying plotting area
    pub fn plotting_area(&self) -> &DrawingArea<DB, CT> {
        &self.drawing_area
    }
}

impl<'a, DB: DrawingBackend, CT: ReverseCoordTranslate> ChartContext<'a, DB, CT> {
    /// Convert the chart context into an closure that can be used for coordinate translation
    pub fn into_coord_trans(self) -> impl Fn(BackendCoord) -> Option<CT::From> {
        let coord_spec = self.drawing_area.into_coord_spec();
        move |coord| coord_spec.reverse_translate(coord)
    }
}

impl<'a, DB: DrawingBackend, X: Ranged, Y: Ranged> ChartContext<'a, DB, RangedCoord<X, Y>> {
    /// Get the range of X axis
    pub fn x_range(&self) -> Range<X::ValueType> {
        self.drawing_area.get_x_range()
    }

    /// Get range of the Y axis
    pub fn y_range(&self) -> Range<Y::ValueType> {
        self.drawing_area.get_y_range()
    }

    /// Maps the coordinate to the backend coordinate. This is typically used
    /// with an interactive chart.
    pub fn backend_coord(&self, coord: &(X::ValueType, Y::ValueType)) -> BackendCoord {
        self.drawing_area.map_coordinate(coord)
    }

    pub(super) fn draw_series_impl<E, R, S>(
        &mut self,
        series: S,
    ) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>>
    where
        for<'b> &'b E: PointCollection<'b, (X::ValueType, Y::ValueType)>,
        E: Drawable<DB>,
        R: Borrow<E>,
        S: IntoIterator<Item = R>,
    {
        for element in series {
            self.drawing_area.draw(element.borrow())?;
        }
        Ok(())
    }

    pub(super) fn alloc_series_anno(&mut self) -> &mut SeriesAnno<'a, DB> {
        let idx = self.series_anno.len();
        self.series_anno.push(SeriesAnno::new());
        &mut self.series_anno[idx]
    }

    /// Draw a data series. A data series in Plotters is abstracted as an iterator of elements
    pub fn draw_series<E, R, S>(
        &mut self,
        series: S,
    ) -> Result<&mut SeriesAnno<'a, DB>, DrawingAreaErrorKind<DB::ErrorType>>
    where
        for<'b> &'b E: PointCollection<'b, (X::ValueType, Y::ValueType)>,
        E: Drawable<DB>,
        R: Borrow<E>,
        S: IntoIterator<Item = R>,
    {
        self.draw_series_impl(series)?;
        Ok(self.alloc_series_anno())
    }

    /// The actual function that draws the mesh lines.
    /// It also returns the label that suppose to be there.
    fn draw_mesh_lines<FmtLabel>(
        &mut self,
        (r, c): (usize, usize),
        (x_mesh, y_mesh): (bool, bool),
        mesh_line_style: &ShapeStyle,
        mut fmt_label: FmtLabel,
    ) -> Result<(Vec<(i32, String)>, Vec<(i32, String)>), DrawingAreaErrorKind<DB::ErrorType>>
    where
        FmtLabel: FnMut(&MeshLine<X, Y>) -> Option<String>,
    {
        let mut x_labels = vec![];
        let mut y_labels = vec![];
        self.drawing_area.draw_mesh(
            |b, l| {
                let draw;
                match l {
                    MeshLine::XMesh((x, _), _, _) => {
                        if let Some(label_text) = fmt_label(&l) {
                            x_labels.push((x, label_text));
                        }
                        draw = x_mesh;
                    }
                    MeshLine::YMesh((_, y), _, _) => {
                        if let Some(label_text) = fmt_label(&l) {
                            y_labels.push((y, label_text));
                        }
                        draw = y_mesh;
                    }
                };
                if draw {
                    l.draw(b, mesh_line_style)
                } else {
                    Ok(())
                }
            },
            r,
            c,
        )?;
        Ok((x_labels, y_labels))
    }

    fn draw_axis_and_labels(
        &self,
        area: Option<&DrawingArea<DB, Shift>>,
        axis_style: Option<&ShapeStyle>,
        labels: &[(i32, String)],
        label_style: &TextStyle,
        label_offset: i32,
        orientation: (i16, i16),
        axis_desc: Option<(&str, &TextStyle)>,
    ) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>> {
        let area = if let Some(target) = area {
            target
        } else {
            return Ok(());
        };

        let (x0, y0) = self.drawing_area.get_base_pixel();

        /* TODO: make this configure adjustable */
        let knob_size = 5;
        let label_dist = if orientation.1 > 0 { 0 } else { 10 };

        let (tw, th) = area.dim_in_pixel();

        let mut axis_range = if orientation.0 == 0 {
            self.drawing_area.get_x_axis_pixel_range()
        } else {
            self.drawing_area.get_y_axis_pixel_range()
        };

        if orientation.0 == 0 {
            axis_range.start -= x0;
            axis_range.end -= x0;
        } else {
            axis_range.start -= y0;
            axis_range.end -= y0;
        }

        if let Some(style) = axis_style {
            let mut x0 = if orientation.0 > 0 { 0 } else { tw as i32 };
            let mut y0 = if orientation.1 > 0 { 0 } else { th as i32 };
            let mut x1 = if orientation.0 >= 0 { 0 } else { tw as i32 };
            let mut y1 = if orientation.1 >= 0 { 0 } else { th as i32 };

            if orientation.0 == 0 {
                x0 = axis_range.start;
                x1 = axis_range.end;
            } else {
                y0 = axis_range.start;
                y1 = axis_range.end;
            }
            area.draw(&Path::new(vec![(x0, y0), (x1, y1)], style.clone()))?;
        }

        let right_most = if orientation.0 > 0 && orientation.1 == 0 {
            labels
                .iter()
                .map(|(_, t)| label_style.font.box_size(t).unwrap_or((0, 0)).0)
                .max()
                .unwrap_or(0) as i32
                + label_dist as i32
        } else {
            0
        };

        for (p, t) in labels {
            let rp = if orientation.0 == 0 { *p - x0 } else { *p - y0 };

            if rp < axis_range.start.min(axis_range.end)
                || axis_range.end.max(axis_range.start) < rp
            {
                continue;
            }

            let (w, h) = label_style.font.box_size(&t).unwrap_or((0, 0));

            let (cx, cy) = match orientation {
                (dx, dy) if dx > 0 && dy == 0 => (right_most - w as i32, *p - y0),
                (dx, dy) if dx < 0 && dy == 0 => (tw as i32 - label_dist - w as i32, *p - y0),
                (dx, dy) if dx == 0 && dy > 0 => (*p - x0, label_dist + h as i32),
                (dx, dy) if dx == 0 && dy < 0 => (*p - x0, th as i32 - label_dist - h as i32),
                _ => panic!("Bug: Invlid orientation specification"),
            };

            let should_draw = if orientation.0 == 0 {
                cx >= 0 && cx + label_offset + w as i32 / 2 <= tw as i32
            } else {
                cy >= 0 && cy + label_offset + h as i32 / 2 <= th as i32
            };

            if should_draw {
                let (text_x, text_y) = if orientation.0 == 0 {
                    (cx - w as i32 / 2 + label_offset, cy)
                } else {
                    (cx, cy - h as i32 / 2 + label_offset)
                };

                area.draw_text(&t, label_style, (text_x, text_y))?;

                if let Some(style) = axis_style {
                    let (kx0, ky0, kx1, ky1) = match orientation {
                        (dx, dy) if dx > 0 && dy == 0 => (0, *p - y0, knob_size, *p - y0),
                        (dx, dy) if dx < 0 && dy == 0 => {
                            (tw as i32 - knob_size, *p - y0, tw as i32, *p - y0)
                        }
                        (dx, dy) if dx == 0 && dy > 0 => (*p - x0, 0, *p - x0, knob_size),
                        (dx, dy) if dx == 0 && dy < 0 => {
                            (*p - x0, th as i32 - knob_size, *p - x0, th as i32)
                        }
                        _ => panic!("Bug: Invlid orientation specification"),
                    };
                    let line = Path::new(vec![(kx0, ky0), (kx1, ky1)], style.clone());
                    area.draw(&line)?;
                }
            }
        }

        if let Some((text, style)) = axis_desc {
            let actual_style = if orientation.0 == 0 {
                style.clone()
            } else {
                if orientation.0 == -1 {
                    style.transform(FontTransform::Rotate270)
                } else {
                    style.transform(FontTransform::Rotate90)
                }
            };

            let (w, h) = actual_style.font.box_size(text).unwrap_or((0, 0));

            let (x0, y0) = match orientation {
                (dx, dy) if dx > 0 && dy == 0 => (tw - w, (th - h) / 2),
                (dx, dy) if dx < 0 && dy == 0 => (0, (th - h) / 2),
                (dx, dy) if dx == 0 && dy > 0 => ((tw - w) / 2, th - h),
                (dx, dy) if dx == 0 && dy < 0 => ((tw - w) / 2, 0),
                _ => panic!("Bug: Invlid orientation specification"),
            };

            area.draw_text(&text, &actual_style, (x0 as i32, y0 as i32))?;
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_mesh<FmtLabel>(
        &mut self,
        (r, c): (usize, usize),
        mesh_line_style: &ShapeStyle,
        label_style: &TextStyle,
        fmt_label: FmtLabel,
        x_mesh: bool,
        y_mesh: bool,
        x_label_offset: i32,
        y_label_offset: i32,
        x_axis: bool,
        y_axis: bool,
        axis_style: &ShapeStyle,
        axis_desc_style: &TextStyle,
        x_desc: Option<String>,
        y_desc: Option<String>,
    ) -> Result<(), DrawingAreaErrorKind<DB::ErrorType>>
    where
        FmtLabel: FnMut(&MeshLine<X, Y>) -> Option<String>,
    {
        let (x_labels, y_labels) =
            self.draw_mesh_lines((r, c), (x_mesh, y_mesh), mesh_line_style, fmt_label)?;

        for idx in 0..2 {
            self.draw_axis_and_labels(
                self.x_label_area[idx].as_ref(),
                if x_axis { Some(axis_style) } else { None },
                &x_labels[..],
                label_style,
                x_label_offset,
                (0, -1 + idx as i16 * 2),
                x_desc.as_ref().map(|desc| (&desc[..], axis_desc_style)),
            )?;

            self.draw_axis_and_labels(
                self.y_label_area[idx].as_ref(),
                if y_axis { Some(axis_style) } else { None },
                &y_labels[..],
                label_style,
                y_label_offset,
                (-1 + idx as i16 * 2, 0),
                y_desc.as_ref().map(|desc| (&desc[..], axis_desc_style)),
            )?;
        }

        Ok(())
    }

    /// Attach a secondary coord to the chart
    pub fn set_secondary_coord<SX: AsRangedCoord, SY: AsRangedCoord>(
        self,
        x_coord: SX,
        y_coord: SY,
    ) -> DualCoordChartContext<
        'a,
        DB,
        RangedCoord<X, Y>,
        RangedCoord<SX::CoordDescType, SY::CoordDescType>,
    > {
        let mut pixel_range = self.drawing_area.get_pixel_range();
        pixel_range.1 = pixel_range.1.end..pixel_range.1.start;

        DualCoordChartContext::new(self, RangedCoord::new(x_coord, y_coord, pixel_range))
    }
}
