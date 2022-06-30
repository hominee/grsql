#[macro_use]
extern crate diesel;
mod entity;
mod schema;

use streamer::*;
use tonic::Request;
use tonic_sqlite::crud_client::CrudClient;
use tonic_sqlite::*;

use std::io::Read;

pub mod tonic_sqlite {
    tonic::include_proto!("tonic_sqlite");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = CrudClient::connect("http://127.0.0.1:7749").await?;

    let file = std::fs::File::open("./input.txt").unwrap();
    let s = Streaming::from(file).chunks(16).then(|chunk| async {
        Data {
            id: None, // it is okay to set None,
            name: "info".into(),
            mime: "text/file".into(),
            created: None,
            updated: None,
            content: chunk,
        }
    });
    let res_create = client.create(s).await?.into_inner();
    let id = res_create.id;
    assert!(res_create.ok, "file not inserted");

    // we query the inserted data
    let query_read = DataQuery {
        query: format!("select * from grsql where id = {};", id),
    };
    let req_read = Request::new(query_read);
    let res_read = client.read(req_read).await?.into_inner();
    let buf = res_read
        .map(|data| {
            assert!(data.is_ok(), "chunk corrupted");
            data.unwrap().results.unwrap().content
        })
        .flat_map(|en| futures_util::stream::iter(en))
        .collect::<Vec<u8>>()
        .await;
    let file = std::fs::File::open("./input.txt").unwrap();
    assert_eq!(
        file.bytes().map(|e| e.unwrap()).collect::<Vec<u8>>(),
        buf,
        "file not consistent"
    );

    // update the inserted data
    let query_update = DataUpdate {
        update: format!("update grsql set id = 9 where id = {};", id),
        content: None,
    };
    let req_update = Request::new(query_update);
    let res_update = client.update(req_update).await?.into_inner();
    assert!(res_update.ok, "id not updated");
    assert_eq!(res_update.rows, 1, "not one row changed");

    // delete the data
    let query_delete = DataQuery {
        query: "delete from grsql where id = 9;".into(),
    };
    let req_delete = Request::new(query_delete);
    let res_delete = client.delete(req_delete).await?.into_inner();
    assert!(res_delete.ok, "data not deleted");
    assert_eq!(res_delete.rows, 1, "not one row deleted");

    Ok(())
}
