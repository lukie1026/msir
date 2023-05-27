use crate::config::ApiConfig;
use anyhow::Result;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use msir_service::{
    statistic::{ConnStat, ConnToStatChanTx, StatEvent, StreamStat, SummariesStat},
    stream::ConnToMgrChanTx,
};
use serde_derive::Serialize;
use std::collections::HashMap;
use tokio::sync::oneshot;
use tracing::info;

#[derive(Debug, Serialize)]
struct ApiResp {
    code: i32,
    data: ApiRespData,
}

#[derive(Debug, Serialize)]
enum ApiRespData {
    #[serde(rename(serialize = "error"))]
    Error(String),

    #[serde(rename(serialize = "root"))]
    Root(HashMap<String, String>),

    #[serde(rename(serialize = "summaries"))]
    Summaries(SummariesStat),

    #[serde(rename(serialize = "clients"))]
    Clients(HashMap<String, ConnStat>),

    #[serde(rename(serialize = "streams"))]
    Streams(HashMap<String, StreamStat>),
}

pub async fn api_server_start(
    stream_tx: ConnToMgrChanTx,
    stat_tx: ConnToStatChanTx,
    config: &ApiConfig,
) -> Result<()> {
    if !config.enabled {
        return Ok(());
    }

    let app = Router::new().nest(
        "/api/v1",
        Router::new()
            .route("/", get(api_root))
            .route("/summaries", get(api_summaries))
            .route("/clients", get(api_clients))
            .route("/client/:cid", get(api_client_byid))
            .route("/streams", get(api_streams))
            .route("/stream/:sid", get(api_stream_byid))
            .with_state((stream_tx, stat_tx)),
    );

    let addr = config.listen.clone().unwrap().parse()?;
    info!("Listening on: {}", addr);
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

async fn api_root() -> impl IntoResponse {
    let mut urls = HashMap::new();
    urls.insert(
        "/summaries".to_string(),
        "the summaries info of instance".to_string(),
    );
    urls.insert(
        "/clients".to_string(),
        "all of the clients info of instance".to_string(),
    );
    urls.insert(
        "/client/:cid".to_string(),
        "the specified client info of instance".to_string(),
    );
    urls.insert(
        "/streams".to_string(),
        "all of the streams info of instance".to_string(),
    );
    urls.insert(
        "/stream/:sid".to_string(),
        "the specified stream info of instance".to_string(),
    );
    Json(ApiResp {
        code: 0,
        data: ApiRespData::Root(urls),
    })
}

async fn api_summaries(
    State((_, stat_tx)): State<(ConnToMgrChanTx, ConnToStatChanTx)>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let query = StatEvent::QuerySummaries(tx);

    if let Err(_) = stat_tx.send(query) {
        return Json(ApiResp {
            code: -1,
            data: ApiRespData::Error("internal error".to_string()),
        });
    }

    match rx.await {
        Ok(res) => Json(ApiResp {
            code: 0,
            data: ApiRespData::Summaries(res),
        }),
        Err(_) => {
            return Json(ApiResp {
                code: -1,
                data: ApiRespData::Error("internal error".to_string()),
            })
        }
    }
}

async fn api_clients(
    State((_, stat_tx)): State<(ConnToMgrChanTx, ConnToStatChanTx)>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let query = StatEvent::QueryConn(String::new(), tx);

    if let Err(_) = stat_tx.send(query) {
        return Json(ApiResp {
            code: -1,
            data: ApiRespData::Error("internal error".to_string()),
        });
    }

    match rx.await {
        Ok(res) => Json(ApiResp {
            code: 0,
            data: ApiRespData::Clients(res),
        }),
        Err(_) => {
            return Json(ApiResp {
                code: -1,
                data: ApiRespData::Error("internal error".to_string()),
            })
        }
    }
}

async fn api_client_byid(
    Path(cid): Path<String>,
    State((_, stat_tx)): State<(ConnToMgrChanTx, ConnToStatChanTx)>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let query = StatEvent::QueryConn(cid, tx);

    if let Err(_) = stat_tx.send(query) {
        return Json(ApiResp {
            code: -1,
            data: ApiRespData::Error("internal error".to_string()),
        });
    }

    match rx.await {
        Ok(res) => Json(ApiResp {
            code: 0,
            data: ApiRespData::Clients(res),
        }),
        Err(_) => {
            return Json(ApiResp {
                code: -1,
                data: ApiRespData::Error("internal error".to_string()),
            })
        }
    }
}

async fn api_streams(
    State((_, stat_tx)): State<(ConnToMgrChanTx, ConnToStatChanTx)>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let query = StatEvent::QueryStream(String::new(), tx);

    if let Err(_) = stat_tx.send(query) {
        return Json(ApiResp {
            code: -1,
            data: ApiRespData::Error("internal error".to_string()),
        });
    }

    match rx.await {
        Ok(res) => Json(ApiResp {
            code: 0,
            data: ApiRespData::Streams(res),
        }),
        Err(_) => {
            return Json(ApiResp {
                code: -1,
                data: ApiRespData::Error("internal error".to_string()),
            })
        }
    }
}

async fn api_stream_byid(
    Path(sid): Path<String>,
    State((_, stat_tx)): State<(ConnToMgrChanTx, ConnToStatChanTx)>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let query = StatEvent::QueryStream(sid, tx);

    if let Err(_) = stat_tx.send(query) {
        return Json(ApiResp {
            code: -1,
            data: ApiRespData::Error("internal error".to_string()),
        });
    }

    match rx.await {
        Ok(res) => Json(ApiResp {
            code: 0,
            data: ApiRespData::Streams(res),
        }),
        Err(_) => {
            return Json(ApiResp {
                code: -1,
                data: ApiRespData::Error("internal error".to_string()),
            })
        }
    }
}
