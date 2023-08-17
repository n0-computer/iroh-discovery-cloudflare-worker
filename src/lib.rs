use worker::*;
use serde::{Deserialize, Serialize};
use serde_json::to_string;
use futures::future::join_all; 

// This is a shared data struct that we will pass to the router
struct SharedData {
    name: String,
}

// This is the struct that we will use to store and retrieve data from KV. It implements Serialize and Deserialize
#[derive(Clone, Debug, Deserialize, Serialize)]
struct AnimalRescue {
    id: u8,
    name: String,
    age: u8,
    species: String,
}

// This is the struct that we will use to update data in KV. It implements Serialize and Deserialize
#[derive(Clone, Debug, Deserialize, Serialize)]
struct AnimalRescueUpdate {
    name: String,
    age: u8,
    species: String,
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // Shared data is accessible across requests
    let shared_data = SharedData {
        name: "Rustacean".to_string(),
    };

    // Create a new router with the shared data
    let router = Router::with_data(shared_data);

    // Router definition
    router
        .get("/shared-data", |_, ctx| {
             // Get the shared data from the context. This is available because we used with_data above.
            let shared_data = ctx.data.name;
            // Return the response
            Response::ok(shared_data)
        })
        .post_async("/rescues", |mut req, ctx| async move {
             // Get the KV namespace
            let kv = ctx.kv("Animal_Rescues_Rusty_KV")?;
            // Get the body of the request - Note that AnimalRescue implements Deserialize
            let body = req.json::<AnimalRescue>().await?;
            // Serialize the body to a string
            let value = to_string(&body)?;
            // Store the value in KV
            kv.put(&body.id.to_string(), value)?.execute().await?;
            // Return the response
            Response::from_json(&body)
        })
        .delete_async("/rescues/:id", |_req, ctx| async move {
            // Get the id from the request, we use if let to check if the id exists
            if let Some(id) = ctx.param("id") {
                // Get the KV namespace
                let kv = ctx.kv("Animal_Rescues_Rusty_KV")?;
                // Delete the value from KV. In this case, 
                // we use the id as the key and return a match statement in case of an error.
                return match kv.delete(id).await {
                    // ! NOTE: I could not find a way to return a 204 No Content response, so this has an empty body.
                    Ok(_) => Response::ok("").map(|resp| resp.with_status(204)),
                    Err(e) => Response::error(e.to_string(), 404)
                };
            }
            Response::error("Animal not found", 404)
        })
        .put_async("/rescues/:id", |mut req, ctx| async move {
            // Get the id from the request, we use if let to check if the id exists
            if let Some(id) = ctx.param("id") {
                // Get the KV namespace
                let kv = ctx.kv("Animal_Rescues_Rusty_KV")?;
                // Get the body of the request - Note that AnimalRescueUpdate implements Deserialize
                let body = req.json::<AnimalRescueUpdate>().await?;
                // Check to see if the id exists in KV
                if kv.get(id).json::<AnimalRescue>().await?.is_none() {
                    // If the id does not exist, return an error
                    return Response::error("Animal not found", 404);
                }

                // Create a new AnimalRescue struct from the body and id
                let new_animal = AnimalRescue {
                    id: id.parse::<u8>().unwrap(),
                    name: body.name,
                    age: body.age,
                    species: body.species,
                };

                // Serialize new_animal to a string
                let value = to_string(&new_animal)?;
                // Store the value in KV
                kv.put(&id, value)?.execute().await?;
                // Return the response
                return Response::from_json(&new_animal);
            }
            Response::error("Animal not found", 404)
        })
        .get_async("/rescues/:id", |_req, ctx | async move {
            // Get the id from the request, we use if let to check if the id exists
            if let Some(id) = ctx.param("id") {
                // Get the KV namespace
                let kv = ctx.kv("Animal_Rescues_Rusty_KV")?;
                // Get the value from KV. In this case, 
                // we use the id as the key and return a match statement because the value may not exist.
                return match kv.get(id).json::<AnimalRescue>().await? {
                    Some(animal) => Response::from_json(&animal),
                    None => Response::error("Animal not found", 404)
                };
            }
            Response::error("Animal not found", 404)
        })
        .get_async("/rescues", |_req, ctx | async move {
            // Get the KV namespace
            let kv = ctx.kv("Animal_Rescues_Rusty_KV")?;

            // Get all the keys from KV
            let keys = kv
                .list()
                .execute()
                .await?
                .keys;
        
            console_debug!("{:?}", keys);

            // Create a Vec of only the key names
            let key_names = keys
                .into_iter()
                .map(|key| key.name)
                .collect::<Vec<String>>();

            console_debug!("{:?}", key_names);

            // Create a Vec of the futures, each future will return an AnimalRescue from KV.

            // The JavaScript code most comprarable to this is:
            // -----------------------------------------------
            // const values = keys.map(key => key.name);
            // const futures = values.map(key => kv.get(key).json());
            // const animals = await Promise.all(futures);
            // const final_result = new Response(JSON.stringify(animals));
            // return final_result;
            // -----------------------------------------------

            let futures = key_names
                .iter()
                .map(|key| kv.get(key).json::<AnimalRescue>());

            // Wait for all the futures to complete. This is similar to Promise.all in JavaScript.
            let animals = join_all(futures)
                .await
                .into_iter()
                .filter_map(|animal| animal.ok())
                .collect::<Vec<_>>().into_iter()
                .map(|animal| animal)
                .collect::<Vec<_>>();
 
            // Create a response from the animals Vec, wrapped in a Result type.
            let final_result = Response::from_json(&animals);
            console_debug!("Final Result: \n {:?}", &final_result);              
            
            final_result
        })
        .run(req, env).await
}
