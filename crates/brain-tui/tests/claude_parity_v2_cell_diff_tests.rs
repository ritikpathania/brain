use brain_tui::state::{ConnectionMode, UiState};
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::dark_theme;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

/// Helper to check if a row contains outer frame corner glyphs: ┌, ┐, └, ┘
fn row_contains_frame_corners(buffer: &ratatui::buffer::Buffer, y: u16, width: u16) -> bool {
    let frame_corners = ['┌', '┐', '└', '┘'];
    for x in 0..width {
        let sym = buffer.get(x, y).symbol();
        if sym.chars().any(|c| frame_corners.contains(&c)) {
            return true;
        }
    }
    false
}

#[test]
fn test_claude_parity_v2_phase1_canvas_unboxing_cell_buffer() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();
    let (w, h) = (80, 24);
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = UiState::new();
    state.terminal_width = w;
    state.terminal_height = h;
    state.connection_mode = ConnectionMode::Daemon;

    terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme)).unwrap();
    let buffer = terminal.backend().buffer();

    // Strict assertion 1: Top row (y=0) of conversation canvas must NOT contain outer border frame glyphs (┌, ─, ┐)
    assert!(
        !row_contains_frame_corners(buffer, 0, w),
        "Canvas floor row y=0 contains outer container border chrome!"
    );
}

#[test]
fn test_claude_parity_v2_phase2_status_footer_geometry() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();
    let (w, h) = (80, 24);
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = UiState::new();
    state.terminal_width = w;
    state.terminal_height = h;
    state.screen = brain_tui::ui::navigation::Screen::Home;

    terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme)).unwrap();
    let buffer = terminal.backend().buffer();

    // Footer must sit exactly at y = height - 1 (row 23) without borders
    assert!(!row_contains_frame_corners(buffer, 23, w));
    let mut row_23_text = String::new();
    for x in 0..w {
        row_23_text.push_str(buffer.get(x, 23).symbol());
    }
    assert!(
        row_23_text.contains("shortcuts")
            || row_23_text.contains("manual mode on")
            || row_23_text.contains("help")
            || row_23_text.contains("commands")
    );
}

#[test]
fn test_claude_parity_v2_phase3_ephemeral_workspace_stream_width() {
    let renderer = AppRenderer::new();
    let (w, h) = (80, 24);
    let mut state = UiState::new();
    state.terminal_width = w;
    state.terminal_height = h;

    // Strict assertion: On Screen::Workspace, collapsed sidebar state MUST allocate 0 sidebar width and 80 chat width
    state.screen = brain_tui::ui::navigation::Screen::Workspace;
    let (_, sb_area, chat_area, _, _, _, _) = renderer.compute_layout(Rect::new(0, 0, w, h), &state);

    assert_eq!(sb_area.width, 0, "Sidebar allocates 22 columns by default in Workspace mode!");
    assert_eq!(chat_area.width, 80, "Conversation stream does not receive 100% width!");
}

#[test]
fn test_claude_parity_v2_phase4_continuous_canvas_transition() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();
    let (w, h) = (80, 24);
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = UiState::new();
    state.terminal_width = w;
    state.terminal_height = h;

    terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme)).unwrap();
    let buffer = terminal.backend().buffer();

    // Home logo canvas floor y=0 must be borderless (no ┌ ─ ┐ container box)
    assert!(
        !row_contains_frame_corners(buffer, 0, w),
        "Home mascot stage renders inside outer container box!"
    );
}

#[test]
fn test_claude_parity_v2_phase5_floating_command_palette_overlay() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();
    let (w, h) = (80, 24);
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = UiState::new();
    state.terminal_width = w;
    state.terminal_height = h;
    state.viewport.is_command_palette_open = true;

    terminal.draw(|f| renderer.draw(f, f.size(), &state, &theme)).unwrap();
    let buffer = terminal.backend().buffer();

    // Palette area (y=16..20) must float directly above prompt, NOT centered modal at y=3..18
    let mid_y_cell = buffer.get(40, 5); // Row 5 (centered modal area)
    assert_ne!(
        mid_y_cell.symbol(),
        "┌",
        "Command palette uses old centered modal box geometry instead of floating prompt dropdown!"
    );
}

#[test]
fn test_claude_parity_v2_phase6_inline_collapsible_memory_chips() {
    let item = brain_domain::retrieval::EvidenceItem {
        id: brain_domain::retrieval::EvidenceId::new(),
        document: brain_domain::DocumentId::new(),
        source: brain_domain::SourceId("brain-domain".to_string()),
        excerpt: "Relational graph memory".to_string(),
        line_range: Some((1, 10)),
        score: 0.95,
        weight: brain_domain::RetrievalWeight::High,
        explanation: brain_domain::retrieval::StructuredRetrievalExplanation {
            reasons: vec![],
            final_rank: 1,
        },
    };

    let card = brain_tui::ui::widgets::evidence_card::EvidenceCard {
        item: &item,
        index: 0,
        is_selected: false,
    };

    let (w, h) = (80, 5);
    let mut buffer = ratatui::buffer::Buffer::empty(Rect::new(0, 0, w, h));
    let theme = dark_theme();

    card.render(Rect::new(0, 0, w, h), &mut buffer, &theme);

    // Single-line chip must contain brain marker 🧠 or compact summary, NOT 3-line panel box
    let mut line_0 = String::new();
    for x in 0..w {
        line_0.push_str(buffer.get(x, 0).symbol());
    }
    assert!(
        line_0.contains("🧠") || line_0.contains("Recalled"),
        "EvidenceCard renders dense bordered panel instead of single-line chip 🧠!"
    );
}
