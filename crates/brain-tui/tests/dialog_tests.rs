mod common;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use brain_tui::ui::theme::dark_theme;
use brain_tui::ui::render::{RenderContext, IconSet};
use brain_tui::ui::widgets::view_models::{DialogView, DialogButton, ButtonKind};
use brain_tui::ui::widgets::{Dialog, brain_widget::BrainWidget};

#[test]
fn test_dialog_snapshot() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext { theme, icons: &icons, capabilities, tick: 0 };
    
    let buttons = [
        DialogButton { label: "Confirm", kind: ButtonKind::Primary, enabled: true },
        DialogButton { label: "Cancel", kind: ButtonKind::Secondary, enabled: true },
    ];
    let view = DialogView {
        title: "Danger Zone",
        message: "Are you sure?",
        buttons: &buttons,
        selected_index: 0,
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 6));
    let widget = Dialog { view: &view };
    widget.render(Rect::new(0, 0, 30, 6), &mut buf, &ctx);
    
    common::assert_snapshot(&buf, &ctx, "widgets/dialog");
}
