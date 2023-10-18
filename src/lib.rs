use pkarr::{bytes::Bytes, PublicKey, SignedPacket};
use worker::*;

const INDEX: &str = r"iroh discovery service";

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // Create a new router with the shared data
    let router = Router::with_data(());

    // Router definition
    router
        .get("/", |_req, _ctx| Response::ok(INDEX))
        .get_async("/:id", |_req, ctx| async move {
            let Some(id) = ctx.param("id") else {
                return Response::error("peer id not provided", 404);
            };
            let public_key = match PublicKey::try_from(id.as_str()) {
                Ok(key) => key,
                Err(cause) => {
                    return Response::error(format!("invalid peer id {}: {}", id, cause), 404);
                }
            };
            let kv = ctx.kv("disco")?;
            kv.get(&public_key.to_z32())
                .bytes()
                .await?
                .map(|bytes| Response::from_bytes(bytes))
                .unwrap_or_else(|| Response::error("not found", 404))
        })
        .put_async("/:id", |mut req, ctx| async move {
            let Some(id) = ctx.param("id") else {
                return Response::error("peer id not provided", 404);
            };
            let public_key = match PublicKey::try_from(id.as_str()) {
                Ok(key) => key,
                Err(cause) => {
                    return Response::error(format!("invalid peer id {}: {}", id, cause), 404);
                }
            };
            let Ok(bytes) = req.bytes().await else {
                return Response::error("missing payload", 404);
            };
            let bytes: Bytes = bytes.into();
            let packet = match SignedPacket::from_bytes(public_key, bytes.clone()) {
                Ok(packet) => packet,
                Err(cause) => {
                    return Response::error(format!("invalid packet: {}", cause), 404);
                }
            };
            let kv = ctx.kv("disco")?;
            kv.put_bytes(&packet.public_key().to_z32(), &bytes)?
                .expiration_ttl(60 * 60 * 24 * 7)
                .execute()
                .await?;
            Response::ok("Hello from Rust!")
        })
        .run(req, env)
        .await
}
