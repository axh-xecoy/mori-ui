#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio;
mod math;
mod model;
mod provider;
mod storage;
#[cfg(test)]
mod test_support;
mod tools;
#[cfg(test)]
mod tools_tests;
mod ui;
use gpui_kit::*;
fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1280.), px(860.)),
                    cx,
                ))),
                window_min_size: Some(size(px(900.), px(640.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Mori · 留一点空间给灵感".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let (_handle, _view) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| ui::Mori::new(window, cx))
            })
            .expect("无法打开 Mori 窗口");
            #[cfg(feature = "visual-test")]
            if let Some(directory) = std::env::var_os("MORI_SNAPSHOT_DIR") {
                ui::capture(_handle, _view, std::path::PathBuf::from(directory), cx);
            }
            cx.activate(true);
        });
}
