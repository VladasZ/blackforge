//! The small lists the blackforge server keeps by hand and serves to anybody,
//! the broken mods and the required ones. Each is cached in the index folder.

use std::{path::Path, time::Duration};

use serde::{Serialize, de::DeserializeOwned};
use tokio::fs;

use crate::{
    error::{IoContext, Result},
    http::Client,
    paths::DataDir,
    social::client::SERVER,
    util::file_age,
};

/// A copy this old is read again from the server.
const MAX_AGE: Duration = Duration::from_hours(1);

/// The list at `route` as the server has it now. The cached copy in `file`
/// serves while it is younger than an hour. When the server cannot be
/// reached, an older copy still serves, the mods of a profile must list
/// without the network. Only a machine with no copy at all gets the error.
pub(crate) async fn load<T: Serialize + DeserializeOwned + Send + 'static>(
    client: &Client,
    data: &DataDir,
    file: &str,
    route: &str,
) -> Result<T> {
    let path = data.index_dir().join(file);
    let age = file_age(&path).await;
    if age.is_some_and(|age| age < MAX_AGE) {
        return read_cache(&path).await;
    }
    match client.get_json::<T>(&format!("{SERVER}{route}")).await {
        Ok(list) => {
            fs::create_dir_all(data.index_dir())
                .await
                .at(&data.index_dir())?;
            fs::write(&path, serde_json::to_vec(&list)?)
                .await
                .at(&path)?;
            Ok(list)
        }
        Err(_) if age.is_some() => read_cache(&path).await,
        Err(error) => Err(error),
    }
}

async fn read_cache<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).await.at(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}
