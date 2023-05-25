use anyhow::Result;
use futures::channel::mpsc::unbounded;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server, StatusCode,
};
use msir_service::httpflv_service::HttpFlvService;
use msir_service::{statistic::ConnToStatChanTx, stream::ConnToMgrChanTx, utils};
use std::{convert::Infallible, io};

use tracing::{error, info, Instrument};

// type FlvRespChanRx = UnboundedReceiver<io::Result<Vec<u8>>>;

pub async fn http_server_start(
    stream_tx: ConnToMgrChanTx,
    stat_tx: ConnToStatChanTx,
) -> Result<()> {
    let addr = ([127, 0, 0, 1], 8091).into();

    info!("Listening on: {}", addr);

    let make_service = make_service_fn(move |_socket| {
        let stream_tx_c = stream_tx.clone();
        let stat_tx_c = stat_tx.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                http_service(
                    req,
                    utils::gen_uid(),
                    stream_tx_c.clone(),
                    stat_tx_c.clone(),
                )
            }))
        }
    });

    Server::bind(&addr).serve(make_service).await?;

    Ok(())
}

async fn http_service(
    req: Request<Body>,
    uid: String,
    stream: ConnToMgrChanTx,
    stat: ConnToStatChanTx,
) -> Result<Response<Body>> {
    if req.uri().path().ends_with(".flv") {
        if let Ok(resp) = httpflv_service(req, uid, stream, stat).await {
            return Ok(resp);
        }
    }

    let resp = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body("404".into())
        .unwrap();
    Ok(resp)
}

async fn httpflv_service(
    req: Request<Body>,
    uid: String,
    stream: ConnToMgrChanTx,
    stat: ConnToStatChanTx,
) -> Result<Response<Body>> {
    let (tx, rx) = unbounded::<io::Result<Vec<u8>>>();

    let mut flv_service = HttpFlvService::new(uid.clone(), tx, stream, stat);
    tokio::spawn(
        async move {
            if let Err(e) = flv_service.run(req).await {
                error!("Failed to transfer; error={}", e);
            }
        }
        .instrument(tracing::info_span!("FLV-CONN", uid)),
    );

    let mut resp = Response::new(Body::wrap_stream(rx));
    resp.headers_mut()
        .insert("Access-Control-Allow-Origin", "*".parse().unwrap());
    Ok(resp)
}