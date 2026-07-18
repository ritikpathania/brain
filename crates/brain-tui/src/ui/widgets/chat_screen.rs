//! ChatScreen layout composer.

use crate::ui::layout::LayoutEngine;
use crate::ui::render::context::RenderContext;
use crate::ui::screen::Screen;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use crate::ui::widgets::view_models::{
    ChatScreenView, ConnectionState, FocusState, FocusTarget, FooterView, PanelView, ShortcutHint,
    StatusBarView, StatusKind,
};
use crate::ui::widgets::{brain_widget::BrainWidget, Footer, Panel, StatusBar};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Composed ChatScreen panel layout orchestrator.
pub struct ChatScreen<'a> {
    /// Reference to the immutable chat screen semantic view model.
    pub view: &'a ChatScreenView<'a>,
}

impl<'a> Screen for ChatScreen<'a> {
    fn render<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let geometry = LayoutEngine::chat_screen(area);

        // 1. Render Status Bar
        let sb_view = map_connection(
            self.view.connection,
            self.view.is_working,
            self.view.session_title,
        );
        let status_bar = StatusBar { view: &sb_view };
        status_bar.render(geometry.status_bar_area, buf, ctx);

        // 2. Render Sidebar Panel (only if standard profile/visible width)
        if geometry.sidebar_area.width > 0 {
            let sb_focused = matches!(self.view.focus, FocusTarget::Sidebar);
            let sidebar_view = map_focus(sb_focused, "Sessions");
            let sidebar_panel = Panel {
                view: &sidebar_view,
            };
            sidebar_panel.render(geometry.sidebar_area, buf, ctx);

            // Draw list mock placeholder content inside panel
            let inner_rect = Rect::new(
                geometry.sidebar_area.x + 1,
                geometry.sidebar_area.y + 1,
                geometry.sidebar_area.width.saturating_sub(2),
                geometry.sidebar_area.height.saturating_sub(2),
            );
            buf.set_stringn(
                inner_rect.x,
                inner_rect.y,
                "• active-session",
                inner_rect.width as usize,
                ctx.theme.style(ThemeToken::Primary),
            );
        }

        // 3. Render Chat Viewport Panel
        let chat_focused = matches!(self.view.focus, FocusTarget::Conversation);
        let chat_view = map_focus(chat_focused, "Conversation");
        let chat_panel = Panel { view: &chat_view };
        chat_panel.render(geometry.chat_viewport_area, buf, ctx);

        // Draw viewport placeholder content
        let chat_inner = Rect::new(
            geometry.chat_viewport_area.x + 1,
            geometry.chat_viewport_area.y + 1,
            geometry.chat_viewport_area.width.saturating_sub(2),
            geometry.chat_viewport_area.height.saturating_sub(2),
        );
        if chat_inner.width > 5 {
            buf.set_stringn(
                chat_inner.x,
                chat_inner.y,
                &format!("Messages count: {}", self.view.message_count),
                chat_inner.width as usize,
                ctx.theme.style(ThemeToken::Muted),
            );
        }

        // 4. Render Prompt Input Panel
        let prompt_focused = matches!(self.view.focus, FocusTarget::Prompt);
        let prompt_view = map_focus(prompt_focused, "Prompt");
        let prompt_panel = Panel { view: &prompt_view };
        prompt_panel.render(geometry.prompt_area, buf, ctx);

        let prompt_inner = Rect::new(
            geometry.prompt_area.x + 1,
            geometry.prompt_area.y + 1,
            geometry.prompt_area.width.saturating_sub(2),
            geometry.prompt_area.height.saturating_sub(2),
        );
        if prompt_inner.width > 2 {
            buf.set_stringn(
                prompt_inner.x,
                prompt_inner.y,
                self.view.input_buffer,
                prompt_inner.width as usize,
                ctx.theme.style(ThemeToken::Primary),
            );
        }

        // 5. Render Footer Panel
        let shortcuts = [
            ShortcutHint {
                key: "Esc",
                description: "Focus Mode",
            },
            ShortcutHint {
                key: "Ctrl+C",
                description: "Exit",
            },
        ];
        let footer_view = FooterView {
            shortcuts: &shortcuts,
        };
        let footer = Footer { view: &footer_view };
        footer.render(geometry.footer_area, buf, ctx);
    }

    fn title(&self) -> &'static str {
        "Chat"
    }
}

/// Helper mapping daemon ConnectionState and working flags to widget StatusBarViews.
fn map_connection(state: ConnectionState, working: bool, title: &str) -> StatusBarView<'_> {
    let (kind, msg) = match state {
        ConnectionState::Connected => (
            if working {
                StatusKind::Working
            } else {
                StatusKind::Idle
            },
            "Connected",
        ),
        ConnectionState::Connecting => (StatusKind::Working, "Connecting..."),
        ConnectionState::Offline => (StatusKind::Offline, "Offline"),
        ConnectionState::Error => (StatusKind::Error, "Connection Error"),
    };
    StatusBarView {
        title,
        kind,
        message: msg,
    }
}

/// Helper mapping focus target booleans to widget PanelViews.
fn map_focus(panel_focus: bool, title: &str) -> PanelView<'_> {
    PanelView {
        title,
        focus: if panel_focus {
            FocusState::Focused
        } else {
            FocusState::Inactive
        },
    }
}
