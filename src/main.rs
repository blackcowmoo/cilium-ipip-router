#![allow(unused_imports, unused_variables)]
use actix_web::{
    get, middleware, web::Data, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::broadcast,
};

#[get("/health")]
async fn health(_: HttpRequest) -> impl Responder {
    HttpResponse::Ok().json("healthy")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    log4rs::init_file("resources/log4rs.yaml", Default::default()).unwrap();

    let controller = cilium_ipip_router::controller::run().await;

    // Start web server
    let server = HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default().exclude("/health"))
            .service(health)
    })
    .bind("0.0.0.0:9090")?
    .shutdown_timeout(30)
    .run();

    let server_handle = server.handle();
    let controller_handle = controller.handle();

    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigint = signal(SignalKind::interrupt()).unwrap();

    log::info!("server started");
    tokio::pin!(server);
    tokio::pin!(controller);

    tokio::select! {
        _ = sigterm.recv() => {
            log::info!("received terminate signal");
            tokio::join!(server_handle.stop(true), controller_handle.stop(true), server, controller).2?;
        }
        _ = sigint.recv() => {
            log::info!("received interrupt signal");
            tokio::join!(server_handle.stop(true), controller_handle.stop(true), server, controller).2?;
        },
        r = &mut server => {
            log::info!("server finished");
            tokio::join!(controller_handle.stop(true),controller).1?;
            r.unwrap();
        },
        r = &mut controller => {
            log::info!("controller finished");
            tokio::join!(server_handle.stop(true), server).1?;
            r.unwrap();
        }
    }

    Ok(())
}
