mod app;
mod route;
mod security;

use app::App;

fn main() {
    yew::Renderer::<App>::new().render();
}
