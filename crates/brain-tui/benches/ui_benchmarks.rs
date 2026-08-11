use brain_tui::state::{IncrementalTokenizer, UiState};
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::Theme;
use criterion::{criterion_group, criterion_main, Criterion};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn bench_frame_draw_empty(c: &mut Criterion) {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let renderer = AppRenderer::new();
    let state = UiState::new();
    let theme = Theme::default();

    c.bench_function("frame_draw_empty_120x40", |b| {
        b.iter(|| {
            terminal
                .draw(|f| {
                    let area = f.size();
                    renderer.draw(f, area, &state, &theme);
                })
                .unwrap();
        });
    });
}

fn bench_tokenizer_throughput(c: &mut Criterion) {
    let mut tokenizer = IncrementalTokenizer::new();
    let chunk = "The brain project uses SQLite runtime DB and Unix Domain Sockets for IPC protocol streaming.\n";

    c.bench_function("tokenizer_feed_chunk", |b| {
        b.iter(|| {
            tokenizer.feed(chunk);
        });
    });
}

criterion_group!(benches, bench_frame_draw_empty, bench_tokenizer_throughput);
criterion_main!(benches);
