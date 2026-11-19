use axum::Router;
use axum::body::Body;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use tokio::runtime::Handle;
use tokio::task::LocalSet;
use tokio_stream::StreamExt;
use tower_http::services::ServeDir;

async fn root() -> impl IntoResponse {
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::task::spawn_blocking(|| {
        let handle = Handle::current();
        handle.block_on(async move {
            let local = LocalSet::new();
            local.spawn_local(async move {
                tx.send(sycamore::render_to_string_stream(crate::app::Main))
                    .ok()
                    .expect("reciever is alive");
            });
            local.await;
        })
    });
    let stream = rx.await.expect("sender is alive");

    let body = Body::from_stream(stream.map(Ok::<_, std::convert::Infallible>));

    ([(header::CONTENT_TYPE, "text/html")], body)
}

async fn root_blocking() -> impl IntoResponse {
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::task::spawn_blocking(|| {
        let handle = Handle::current();
        handle.block_on(async move {
            tx.send(sycamore::render_to_string_await_suspense(crate::app::Main).await)
                .ok()
                .unwrap();
        });
    });
    let body = rx.await.unwrap();

    ([(header::CONTENT_TYPE, "text/html")], body)
}

/// Start the server.
pub async fn start() {
    let current_file = file!();
    let dist_public_path = std::path::Path::new(current_file)
        .parent()
        .unwrap()
        .join("../dist/public");

    let app = Router::new()
        .route("/", get(root))
        .route("/blocking", get(root_blocking))
        .nest_service("/dist/public", ServeDir::new(dist_public_path));

    // run our app with hyper, listening globally on port 8080
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
