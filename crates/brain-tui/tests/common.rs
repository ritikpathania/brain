use brain_tui::ui::render::RenderContext;
use brain_tui::ui::render::{
    ColorSupport, EffectiveCapabilities, MotionPreference, NerdFontsSupport, UnicodeSupport,
};
use brain_tui::ui::theme::ActiveTheme;
use ratatui::buffer::Buffer;

pub fn mock_capabilities() -> EffectiveCapabilities {
    EffectiveCapabilities {
        unicode: UnicodeSupport::Full,
        colors: ColorSupport::TrueColor,
        nerd_fonts: NerdFontsSupport::Full,
        mouse: true,
        osc8: true,
        motion: MotionPreference::Full,
    }
}

pub fn format_buffer<T: ActiveTheme>(buf: &Buffer, ctx: &RenderContext<'_, T>) -> String {
    let mut res = String::new();
    // Capabilities Metadata Header
    res.push_str("--- Metadata ---\n");
    res.push_str("theme=dark\n");
    res.push_str(&format!("unicode={:?}\n", ctx.capabilities.unicode));
    res.push_str(&format!("colors={:?}\n", ctx.capabilities.colors));
    res.push_str(&format!("nerd_fonts={:?}\n", ctx.capabilities.nerd_fonts));
    res.push_str(&format!("width={}\n", buf.area.width));
    res.push_str(&format!("height={}\n", buf.area.height));

    res.push_str("--- Visual ---\n");
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            res.push_str(buf.get(x, y).symbol());
        }
        res.push('\n');
    }
    res.push_str("--- Styles ---\n");
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let cell = buf.get(x, y);
            let fg = cell
                .style()
                .fg
                .map(|c| format!("{:?}", c))
                .unwrap_or("Reset".to_string());
            let bg = cell
                .style()
                .bg
                .map(|c| format!("{:?}", c))
                .unwrap_or("Reset".to_string());
            res.push_str(&format!("({},{}) ", fg, bg));
        }
        res.push('\n');
    }
    res
}

pub fn assert_snapshot<T: ActiveTheme>(buf: &Buffer, ctx: &RenderContext<'_, T>, name: &str) {
    let current = format_buffer(buf, ctx);
    let snap_path = format!("tests/golden/{}.snap", name);
    let path = std::path::Path::new(&snap_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }

    if std::env::var("UPDATE_EXPECT").is_ok() {
        std::fs::write(&snap_path, &current).unwrap();
    } else {
        let expected = std::fs::read_to_string(&snap_path).unwrap_or_else(|_| {
            std::fs::write(&snap_path, &current).unwrap();
            current.clone()
        });
        assert_eq!(current, expected);
    }
}
