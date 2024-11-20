use crate::state::AppState;
use sea_orm::Database;

mod book;
mod deal;
mod msg;
mod period;
mod route;
mod security;
mod state;

#[tokio::main]
async fn main() {
    axum::serve(
        tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap(),
        route::create_router().with_state(
            AppState::restore(
                Database::connect("postgres://mint:security_matching@localhost/security_matching")
                    .await
                    .unwrap(),
            )
            .await,
        ),
    )
    .await
    .unwrap();
}
