use iced::{Color, Rectangle, Renderer, Theme, color, mouse::Cursor, widget::canvas};

pub const MIC_ICON_DISABLED: Color = color!(104.0, 104.0, 104.0);
pub const MIC_ICON_ENABLED: Color = color!(0.0, 128.0, 0.0);

#[derive(Debug)]
pub struct MicIcon {
    pub radius: f32,
    pub color: Color,
}

impl<Message> canvas::Program<Message> for MicIcon {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let circle = canvas::Path::circle(frame.center(), self.radius);
        frame.fill(&circle, self.color);
        vec![frame.into_geometry()]
    }
}
