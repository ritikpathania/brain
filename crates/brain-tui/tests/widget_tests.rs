mod common;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use brain_tui::ui::theme::dark_theme;
use brain_tui::ui::render::{RenderContext, IconSet};
use brain_tui::ui::widgets::view_models::{
    SectionView, TabView, ToolbarView, ListItem, ListView,
    ScrollViewModel, CommandHintView, EmptyStateView
};
use brain_tui::ui::widgets::{
    Section, Toolbar, List, ScrollViewWidget, CommandHint, EmptyState,
    brain_widget::BrainWidget
};

#[test]
fn test_section_snapshot() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext { theme, icons: &icons, capabilities, tick: 0 };
    
    let view = SectionView { title: "Subsystem A", collapsed: false };
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
    let widget = Section { view: &view };
    widget.render(Rect::new(0, 0, 20, 1), &mut buf, &ctx);
    common::assert_snapshot(&buf, &ctx, "widgets/section");
}

#[test]
fn test_toolbar_snapshot() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext { theme, icons: &icons, capabilities, tick: 0 };
    
    let tabs = [
        TabView { title: "Chat", active: true },
        TabView { title: "Files", active: false },
    ];
    let view = ToolbarView { tabs: &tabs };
    let mut buf = Buffer::empty(Rect::new(0, 0, 25, 1));
    let widget = Toolbar { view: &view };
    widget.render(Rect::new(0, 0, 25, 1), &mut buf, &ctx);
    common::assert_snapshot(&buf, &ctx, "widgets/toolbar");
}

#[test]
fn test_list_snapshot() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext { theme, icons: &icons, capabilities, tick: 0 };
    
    let items = [
        ListItem { label: "Item One", selected: true, disabled: false },
        ListItem { label: "Item Two", selected: false, disabled: true },
    ];
    let view = ListView { items: &items };
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 2));
    let widget = List { view: &view };
    widget.render(Rect::new(0, 0, 20, 2), &mut buf, &ctx);
    common::assert_snapshot(&buf, &ctx, "widgets/list");
}

#[test]
fn test_scroll_view_snapshot() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext { theme, icons: &icons, capabilities, tick: 0 };
    
    let lines = ["Line 1", "Line 2", "Line 3"];
    let view = ScrollViewModel { lines: &lines, scroll_offset: 1 };
    let mut buf = Buffer::empty(Rect::new(0, 0, 15, 2));
    let widget = ScrollViewWidget { view: &view };
    widget.render(Rect::new(0, 0, 15, 2), &mut buf, &ctx);
    common::assert_snapshot(&buf, &ctx, "widgets/scroll_view");
}

#[test]
fn test_command_hint_snapshot() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext { theme, icons: &icons, capabilities, tick: 0 };
    
    let view = CommandHintView { command: "/help", usage: "Display help information" };
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 4));
    let widget = CommandHint { view: &view };
    widget.render(Rect::new(0, 0, 30, 4), &mut buf, &ctx);
    common::assert_snapshot(&buf, &ctx, "widgets/command_hint");
}

#[test]
fn test_empty_state_snapshot() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext { theme, icons: &icons, capabilities, tick: 0 };
    
    let view = EmptyStateView {
        title: "No Sessions",
        description: "Create a session to begin.",
        icon: " Ø ",
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 5));
    let widget = EmptyState { view: &view };
    widget.render(Rect::new(0, 0, 30, 5), &mut buf, &ctx);
    common::assert_snapshot(&buf, &ctx, "widgets/empty_state");
}
