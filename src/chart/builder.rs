use super::context::ChartContext;

use crate::coord::{AsRangedCoord, RangedCoord, Shift};
use crate::drawing::backend::DrawingBackend;
use crate::drawing::{DrawingArea, DrawingAreaErrorKind};
use crate::style::TextStyle;

/// The enum used to specify the position of label area
pub enum LabelAreaPosition {
    Top = 0,
    Bottom = 1,
    Left = 2,
    Right = 3,
}

/// The helper object to create a chart context, which is used for the high-level figure drawing
pub struct ChartBuilder<'a, 'b, DB: DrawingBackend> {
    label_area_size: [u32; 4], // [upper, lower, left, right]
    root_area: &'a DrawingArea<DB, Shift>,
    title: Option<(String, TextStyle<'b>)>,
    margin: u32,
}

impl<'a, 'b, DB: DrawingBackend> ChartBuilder<'a, 'b, DB> {
    /// Create a chart builder on the given drawing area
    /// - `root`: The root drawing area
    /// - Returns: The chart builder object
    pub fn on(root: &'a DrawingArea<DB, Shift>) -> Self {
        Self {
            label_area_size: [0; 4],
            root_area: root,
            title: None,
            margin: 0,
        }
    }

    /// Set the margin size of the chart
    /// - `size`: The size of the chart margin. If the chart builder is titled, we don't apply any
    /// margin
    pub fn margin(&mut self, size: u32) -> &mut Self {
        self.margin = size;
        self
    }

    /// Set the size of X label area
    /// - `size`: The height of the x label area, if x is 0, the chart doesn't have the X label area
    pub fn x_label_area_size(&mut self, size: u32) -> &mut Self {
        self.label_area_size[1] = size;
        self
    }

    /// Set the size of the Y label area
    /// - `size`: The width of the Y label area. If size is 0, the chart doesn't have Y label area
    pub fn y_label_area_size(&mut self, size: u32) -> &mut Self {
        self.label_area_size[2] = size;
        self
    }

    /// Set the size of X label area on the top of the chart
    /// - `size`: The height of the x label area, if x is 0, the chart doesn't have the X label area
    pub fn top_x_label_area_size(&mut self, size: u32) -> &mut Self {
        self.label_area_size[0] = size;
        self
    }

    /// Set the size of the Y label area on the right side
    /// - `size`: The width of the Y label area. If size is 0, the chart doesn't have Y label area
    pub fn right_y_label_area_size(&mut self, size: u32) -> &mut Self {
        self.label_area_size[3] = size;
        self
    }

    /// Set a label area size
    /// - `pos`: THe position where the label area locted
    /// - `size`: The size of the label area size
    pub fn set_label_area_size(&mut self, pos: LabelAreaPosition, size: u32) -> &mut Self {
        self.label_area_size[pos as usize] = size;
        self
    }

    /// Set the caption of the chart
    /// - `caption`: The caption of the chart
    /// - `style`: The text style
    /// - Note: If the caption is set, the margin option will be ignored
    pub fn caption<S: AsRef<str>, Style: Into<TextStyle<'b>>>(
        &mut self,
        caption: S,
        style: Style,
    ) -> &mut Self {
        self.title = Some((caption.as_ref().to_string(), style.into()));
        self
    }

    /// Build the chart with a 2D Cartesian coordinate system. The function will returns a chart
    /// context, where data series can be rendered on.
    /// - `x_spec`: The specification of X axis
    /// - `y_spec`: The specification of Y axis
    /// - Returns: A chart context
    #[allow(clippy::type_complexity)]
    pub fn build_ranged<X: AsRangedCoord, Y: AsRangedCoord>(
        &mut self,
        x_spec: X,
        y_spec: Y,
    ) -> Result<
        ChartContext<'a, DB, RangedCoord<X::CoordDescType, Y::CoordDescType>>,
        DrawingAreaErrorKind<DB::ErrorType>,
    > {
        let mut label_areas = [None, None, None, None];

        let mut drawing_area = DrawingArea::clone(self.root_area);

        if self.margin > 0 {
            let s = self.margin as i32;
            drawing_area = drawing_area.margin(s, s, s, s);
        }

        if let Some((ref title, ref style)) = self.title {
            drawing_area = drawing_area.titled(title, style.clone())?;
        }

        let (w, h) = drawing_area.dim_in_pixel();

        let mut actual_drawing_area_pos = [0, h as i32, 0, w as i32];

        for (idx, (dx, dy)) in (0..4).map(|idx| (idx, [(0, -1), (0, 1), (-1, 0), (1, 0)][idx])) {
            let size = self.label_area_size[idx] as i32;

            let split_point = if dx + dy < 0 { size } else { -size };

            actual_drawing_area_pos[idx] += split_point;
        }

        let mut splitted: Vec<_> = drawing_area
            .split_by_breakpoints(
                &actual_drawing_area_pos[2..4],
                &actual_drawing_area_pos[0..2],
            )
            .into_iter()
            .map(|x| Some(x))
            .collect();

        for (src_idx, dst_idx) in [1, 7, 3, 5].iter().zip(0..4) {
            let (h, w) = splitted[*src_idx].as_ref().unwrap().dim_in_pixel();
            if h > 0 && w > 0 {
                std::mem::swap(&mut label_areas[dst_idx], &mut splitted[*src_idx]);
            }
        }

        std::mem::swap(&mut drawing_area, splitted[4].as_mut().unwrap());

        let mut pixel_range = drawing_area.get_pixel_range();
        pixel_range.1 = pixel_range.1.end..pixel_range.1.start;

        let mut x_label_area = [None, None];
        let mut y_label_area = [None, None];

        std::mem::swap(&mut x_label_area[0], &mut label_areas[0]);
        std::mem::swap(&mut x_label_area[1], &mut label_areas[1]);
        std::mem::swap(&mut y_label_area[0], &mut label_areas[2]);
        std::mem::swap(&mut y_label_area[1], &mut label_areas[3]);

        Ok(ChartContext {
            x_label_area: x_label_area,
            y_label_area: y_label_area,
            drawing_area: drawing_area.apply_coord_spec(RangedCoord::new(
                x_spec,
                y_spec,
                pixel_range,
            )),
            series_anno: vec![],
        })
    }
}
