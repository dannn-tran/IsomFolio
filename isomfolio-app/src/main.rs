mod app;
mod view;

use app::App;
use iced::{Size, Theme};

fn main() -> iced::Result {
    let catalog_dir = std::env::args().nth(1);

    iced::application(
        move || App::new(catalog_dir.clone()),
        App::update,
        App::view,
    )
    .title(|app: &App| app.window_title())
    .subscription(App::subscription)
    .theme(|_: &App| Theme::Dark)
    .window_size(Size::new(1280.0, 800.0))
    .run()
}
