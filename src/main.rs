#![allow(unused_imports, unused_variables)]
use actix_web::{
    get, middleware, web::Data, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use cilium_ipip_router::controller::ControllerHandle;
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

    let builder = cilium_ipip_router::controller::Controller::builder();
    let controller_handle = ControllerHandle::new(builder.cmd_tx.clone());

    let controller_task = tokio::spawn(async move {
        let controller = cilium_ipip_router::controller::Controller::new(builder);
        let _ = controller.await;
    });

    let server = HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default().exclude("/health"))
            .service(health)
    })
    .bind("0.0.0.0:9090")?
    .shutdown_timeout(30)
    .run();

    let server_handle = server.handle();

    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigint = signal(SignalKind::interrupt()).unwrap();

    log::info!("server started");
    tokio::pin!(server);

    tokio::select! {
        _ = sigterm.recv() => {
            log::info!("received terminate signal");
            controller_handle.stop(true).await;
            let (_, _, r) = tokio::join!(controller_task, server_handle.stop(true), server);
            r?;
        }
        _ = sigint.recv() => {
            log::info!("received interrupt signal");
            controller_handle.stop(true).await;
            let (_, _, r) = tokio::join!(controller_task, server_handle.stop(true), server);
            r?;
        },
        r = &mut server => {
            log::info!("server finished");
            controller_handle.stop(true).await;
            let (_, r) = tokio::join!(controller_task, server);
            r.unwrap();
        }
    }

    Ok(())
}
