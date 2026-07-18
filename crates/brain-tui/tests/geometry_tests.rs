use brain_tui::ui::layout::{CellWidth, DialogMeasure, LayoutEngine, StatusBarMeasure};
use ratatui::layout::Rect;

#[test]
fn test_status_bar_geometry_happy_path() {
    let area = Rect::new(0, 0, 30, 1);
    let measure = StatusBarMeasure {
        title_width: CellWidth(8),
        show_spinner: true,
    };
    let geometry = LayoutEngine::status_bar(area, &measure);

    assert_eq!(geometry.title_area, Rect::new(0, 0, 8, 1));
    assert_eq!(geometry.spinner_area, Rect::new(8, 0, 2, 1));
    assert_eq!(geometry.status_area, Rect::new(10, 0, 20, 1));
}

#[test]
fn test_status_bar_geometry_boundaries() {
    // Zero-width boundary
    let area = Rect::new(0, 0, 0, 1);
    let measure = StatusBarMeasure {
        title_width: CellWidth(8),
        show_spinner: true,
    };
    let geometry = LayoutEngine::status_bar(area, &measure);
    assert_eq!(geometry.title_area.width, 0);
    assert_eq!(geometry.spinner_area.width, 0);
    assert_eq!(geometry.status_area.width, 0);

    // Overflow title width
    let area = Rect::new(0, 0, 5, 1);
    let measure = StatusBarMeasure {
        title_width: CellWidth(10),
        show_spinner: false,
    };
    let geometry = LayoutEngine::status_bar(area, &measure);
    assert_eq!(geometry.title_area.width, 5);
    assert_eq!(geometry.status_area.width, 0);
}

#[test]
fn test_dialog_geometry_happy_path() {
    let area = Rect::new(0, 0, 40, 10);
    let button_widths = [CellWidth(7), CellWidth(6)];
    let measure = DialogMeasure {
        button_widths: &button_widths,
    };
    let geometry = LayoutEngine::dialog(area, &measure);

    assert_eq!(geometry.inner_area, Rect::new(1, 1, 38, 8));
    assert_eq!(geometry.message_area, Rect::new(1, 1, 38, 1));
    assert_eq!(geometry.button_areas()[0], Rect::new(1, 3, 11, 1)); // 7 + 4 = 11
}

#[test]
fn test_dialog_geometry_boundaries() {
    // Micro-dimensions dialog
    let area = Rect::new(0, 0, 1, 1);
    let button_widths = [CellWidth(5)];
    let measure = DialogMeasure {
        button_widths: &button_widths,
    };
    let geometry = LayoutEngine::dialog(area, &measure);
    assert_eq!(geometry.inner_area.width, 0);
}
