use brain_domain::{Message, MessageId, MessageRole};
use brain_tui::state::{ConnectionMode, UiState};
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::dark_theme;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_final_visual_qa_matrix_coverage() {
    let resolutions = [(80, 24), (96, 24), (120, 30), (182, 53)];
    let renderer = AppRenderer::new();
    let theme = dark_theme();

    for (w, h) in resolutions {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        // 1. Home Stage
        let mut home_state = UiState::new();
        home_state.connection_mode = ConnectionMode::Daemon;
        home_state.terminal_width = w;
        home_state.terminal_height = h;
        home_state.recalculate_viewport();
        terminal
            .draw(|f| renderer.draw(f, f.size(), &home_state, theme))
            .unwrap();

        let home_buffer = terminal.backend().buffer();
        let home_text = home_buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();

        // Line 0 is top border of container frame titled BRAIN v0.1.0
        let first_row: String = (0..w).map(|x| home_buffer.get(x, 0).symbol()).collect();
        assert!(
            home_text.contains("BRAIN") || home_text.contains("Welcome"),
            "Home mascot stage missing on Home at {}x{}",
            w,
            h
        );
        assert!(
            !first_row.contains("● Connected"),
            "Top line on Home must NOT contain top Connected indicator at {}x{}",
            w,
            h
        );

        // Home DOES contain hero Welcome back!, tagline, NO Ready indicator, and single canonical bottom status
        assert!(
            home_text.contains("Welcome back!"),
            "Missing Welcome back! on Home at {}x{}",
            w,
            h
        );
        assert!(
            home_text.contains("Think once. Remember."),
            "Missing tagline on Home at {}x{}",
            w,
            h
        );
        assert!(
            !home_text.contains("Ready"),
            "Home hero must NOT contain redundant Ready indicator at {}x{}",
            w,
            h
        );

        // 2. Workspace with Query
        let mut ws_state = UiState::new();
        ws_state.screen = brain_tui::ui::navigation::Screen::Workspace;
        ws_state.connection_mode = ConnectionMode::Daemon;
        ws_state.terminal_width = w;
        ws_state.terminal_height = h;
        ws_state
            .editor
            .set_text("Explain relational memory engine graph topology");
        ws_state.update(brain_tui::state::Action::SubmitPrompt);
        ws_state.recalculate_viewport();
        terminal
            .draw(|f| renderer.draw(f, f.size(), &ws_state, theme))
            .unwrap();

        let ws_text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            ws_text.contains("Explain relational memory"),
            "Missing query on Workspace at {}x{}",
            w,
            h
        );
        if w >= 80 {
            assert!(
                ws_text.contains(
                    "enter to return · space to reply · ctrl+x to delete · ? for shortcuts"
                ),
                "Workspace footer format mismatch at {}x{}",
                w,
                h
            );
        }

        // 3. Command Discovery (/)
        let mut slash_state = UiState::new();
        slash_state.terminal_width = w;
        slash_state.terminal_height = h;
        slash_state.command_palette_mut().open_with_query(Some("/"));
        slash_state.recalculate_viewport();

        let (_, _, _, _, p_area, pal_area, f_area) =
            renderer.compute_layout(Rect::new(0, 0, w, h), &slash_state);
        assert_eq!(
            f_area.height, 0,
            "Footer must consume zero rows when palette is open at {}x{}",
            w, h
        );
        assert_eq!(
            pal_area.y,
            p_area.y + p_area.height,
            "Palette must start immediately below prompt at {}x{}",
            w,
            h
        );

        terminal
            .draw(|f| renderer.draw(f, f.size(), &slash_state, theme))
            .unwrap();

        let slash_text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            slash_text.contains("/session new"),
            "Missing /session new in slash picker at {}x{}",
            w,
            h
        );
        assert!(
            !slash_text.contains("Commands ────"),
            "Slash picker must NOT contain title box header at {}x{}",
            w,
            h
        );
        assert!(
            !slash_text.contains("UTF-8 | Truecolor"),
            "Footer MUST NOT be rendered when palette is open at {}x{}",
            w,
            h
        );

        // 4. Global Discovery (Ctrl+K)
        let mut ctrlk_state = UiState::new();
        ctrlk_state.terminal_width = w;
        ctrlk_state.terminal_height = h;
        ctrlk_state.command_palette_mut().open_with_query(None);
        ctrlk_state.recalculate_viewport();

        let (_, _, _, _, ctrl_p_area, ctrl_pal_area, ctrl_f_area) =
            renderer.compute_layout(Rect::new(0, 0, w, h), &ctrlk_state);
        assert_eq!(
            ctrl_f_area.height, 0,
            "Ctrl+K footer must consume zero rows at {}x{}",
            w, h
        );
        assert_eq!(
            ctrl_pal_area.y,
            ctrl_p_area.y + ctrl_p_area.height,
            "Ctrl+K palette must start immediately below prompt at {}x{}",
            w,
            h
        );

        terminal
            .draw(|f| renderer.draw(f, f.size(), &ctrlk_state, theme))
            .unwrap();

        let ctrlk_text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            !ctrlk_text.contains("UTF-8 | Truecolor"),
            "Footer MUST NOT be rendered during Ctrl+K at {}x{}",
            w,
            h
        );

        // Escape closes palette and restores footer
        ctrlk_state.command_palette_mut().close();
        let (_, _, _, _, _, _, restored_f_area) =
            renderer.compute_layout(Rect::new(0, 0, w, h), &ctrlk_state);
        assert_eq!(
            restored_f_area.height, 1,
            "Closing palette with Escape must restore footer height to 1 at {}x{}",
            w, h
        );

        // 5. /theme Screen
        let mut theme_state = UiState::new();
        theme_state.terminal_width = w;
        theme_state.terminal_height = h;
        theme_state
            .command_palette_mut()
            .open_with_query(Some("/theme"));
        theme_state.recalculate_viewport();
        terminal
            .draw(|f| renderer.draw(f, f.size(), &theme_state, theme))
            .unwrap();

        // 6. /help Screen
        let mut help_state = UiState::new();
        help_state.terminal_width = w;
        help_state.terminal_height = h;
        help_state
            .command_palette_mut()
            .open_with_query(Some("/help"));
        help_state.recalculate_viewport();
        terminal
            .draw(|f| renderer.draw(f, f.size(), &help_state, theme))
            .unwrap();

        // 7. /status Screen
        let mut status_state = UiState::new();
        status_state.terminal_width = w;
        status_state.terminal_height = h;
        status_state
            .command_palette_mut()
            .open_with_query(Some("/status"));
        status_state.recalculate_viewport();
        terminal
            .draw(|f| renderer.draw(f, f.size(), &status_state, theme))
            .unwrap();

        // 8. Streaming Response Stage
        let mut stream_state = UiState::new();
        stream_state.screen = brain_tui::ui::navigation::Screen::Workspace;
        stream_state.terminal_width = w;
        stream_state.terminal_height = h;
        stream_state.active_messages.push(Message::new(
            MessageId::new(),
            MessageRole::Assistant,
            "Streaming response token buffer...".to_string(),
        ));
        stream_state.recalculate_viewport();
        terminal
            .draw(|f| renderer.draw(f, f.size(), &stream_state, theme))
            .unwrap();

        // 9. Reasoning Progress Stage
        let mut reason_state = UiState::new();
        reason_state.screen = brain_tui::ui::navigation::Screen::Workspace;
        reason_state.terminal_width = w;
        reason_state.terminal_height = h;
        reason_state.recalculate_viewport();
        terminal
            .draw(|f| renderer.draw(f, f.size(), &reason_state, theme))
            .unwrap();

        // 10. Evidence Results Stage
        let mut ev_state = UiState::new();
        ev_state.screen = brain_tui::ui::navigation::Screen::Workspace;
        ev_state.terminal_width = w;
        ev_state.terminal_height = h;
        ev_state.recalculate_viewport();
        terminal
            .draw(|f| renderer.draw(f, f.size(), &ev_state, theme))
            .unwrap();
    }
}
