#[macro_use]
extern crate diesel;
mod entity;
mod schema;

use entity::establish_connection;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use tonic_sqlite::crud_server::{Crud, CrudServer};
use tonic_sqlite::*;

use diesel::{prelude::*, sql_types::*, sqlite::SqliteConnection};
use std::{cell::Cell, env, pin::Pin, sync::RwLock};
use streamer::{Stream, StreamExt};

pub mod tonic_sqlite {
    tonic::include_proto!("tonic_sqlite");
}

pub struct SqliteCruder {
    conn: RwLock<SqliteConnection>,
    base_id: Cell<i32>,
}
unsafe impl Sync for SqliteCruder {}

impl SqliteCruder {
    pub fn new() -> Self {
        use diesel::prelude::*;
        use schema::grsql::dsl::*;

        dotenv::dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("database_url must not be null");
        let conn = SqliteConnection::establish(&database_url)
            .expect(&format!("fail to connect to {}", database_url));
        let base_id = grsql
            .select(id)
            .order(id.desc())
            .get_result::<i32>(&conn)
            .expect("fail to load max id");

        Self {
            conn: RwLock::new(conn),
            base_id: Cell::new(base_id),
        }
    }
}

#[tonic::async_trait]
impl Crud for SqliteCruder {
    async fn create(
        &self,
        data: Request<Streaming<Data>>,
    ) -> Result<Response<ResponseCreate>, Status> {
        use std::collections::LinkedList;
        dotenv::dotenv().ok();
        let buf_len_limit = std::env::var("BUF_LEN")
            .expect("max buffer size must be set")
            .parse()
            .expect("must be valid integer");
        let mut content_v = LinkedList::new();
        let mut is_start = true;
        let mut buf_len = 0;
        let base_id = self.base_id.clone();
        let mut en: Option<tonic_sqlite::Data> = None;
        let mut input_stream = data.into_inner();
        tokio::spawn(async move {
            while let Some(result) = input_stream.next().await {
                match result {
                    Ok(mut v) => {
                        if is_start {
                            is_start = false;
                            let v2 = Vec::new();
                            let c = std::mem::replace(&mut v.content, v2);
                            buf_len += c.len();
                            content_v.push_back(c);
                            en = Some(v);
                        } else {
                            buf_len += v.content.len();
                            content_v.push_back(v.content);
                            //en.as_mut().unwrap().content.extend(v.content);
                        }
                        if buf_len > buf_len_limit {
                            return Err(Status::cancelled("too long input data"));
                        }
                    }
                    Err(e) => {
                        if let Some(io_err) = match_for_io_error(&e) {
                            if io_err.kind() == std::io::ErrorKind::BrokenPipe {
                                eprintln!("\tclient disconnected: broken pipe");
                                break;
                            }
                        }
                    }
                }
            }

            match en {
                None => Err(Status::aborted("Incomplete message or invalid data")),
                Some(mut e) => {
                    use diesel::prelude::*;
                    use schema::grsql::dsl::*;
                    let mut content_vec = Vec::with_capacity(buf_len);
                    content_v.into_iter().for_each(|c| {
                        content_vec.extend(c);
                    });
                    e.content = content_vec;
                    let mut en: entity::Data = e.into();
                    //assert!(!en.content.is_empty(), "content must not be empty");
                    en.id = base_id.get() + 1;
                    diesel::insert_into(grsql)
                        .values(vec![en])
                        .execute(&establish_connection())
                        .unwrap();
                    Ok(Response::new(ResponseCreate {
                        ok: true,
                        id: base_id.get() + 1,
                        desc: "".into(),
                    }))
                }
            }
        })
        .await
        .and_then(|en| {
            self.base_id.set(self.base_id.get() + 1);
            Ok(en)
        })
        .map_err(|e| Status::internal(format!("runtime execution error: {:?}", e)))
        .unwrap()
    }

    type ReadStream = Pin<Box<dyn Stream<Item = Result<ResponseRead, Status>> + Send>>;
    async fn read(
        &self,
        query: Request<DataQuery>,
        //) -> Result<Response<Streaming<ResponseRead>>, Status> {
    ) -> Result<Response<Self::ReadStream>, Status> {
        let query: DataQuery = query.into_inner();
        match diesel::sql_query(query.query)
            .get_results::<entity::Data>(&*self.conn.read().unwrap())
        {
            Ok(data) => {
                let data_vec = data
                    .into_iter()
                    .map(|en| {
                        Ok::<_, Status>(ResponseRead {
                            ok: true,
                            desc: None,
                            results: Some(en.into()),
                        })
                    })
                    .collect::<Vec<_>>();
                let s = futures_util::stream::iter(data_vec);

                Ok(Response::new(Box::pin(s)))
            }
            Err(e) => Err(Status::internal(format!(
                "Invalid query or Not Found: {:?}",
                e
            ))),
        }
    }

    async fn update(&self, query: Request<DataUpdate>) -> Result<Response<ResponseChange>, Status> {
        let update_query = query.into_inner();
        let query = diesel::sql_query(update_query.update);
        match update_query.content {
            Some(byte) => {
                match query
                    .bind::<Binary, _>(byte)
                    .execute(&*self.conn.read().unwrap())
                {
                    Ok(data) => Ok(Response::new(ResponseChange {
                        ok: true,
                        desc: None,
                        rows: data as _,
                    })),
                    Err(e) => Err(Status::not_found(format!(
                        "Invalid query or Not Found: {:?}",
                        e
                    ))),
                    //Err(e) => Err(Status::not_found("Invalid query or Not Found")),
                }
            }
            None => match query.execute(&*self.conn.read().unwrap()) {
                Ok(data) => Ok(Response::new(ResponseChange {
                    ok: true,
                    desc: None,
                    rows: data as _,
                })),
                Err(e) => Err(Status::not_found(format!(
                    "Invalid query or Not Found: {:?}",
                    e
                ))),
            },
        }
    }

    async fn delete(&self, query: Request<DataQuery>) -> Result<Response<ResponseChange>, Status> {
        use diesel::prelude::*;

        let delete_query = query.into_inner();
        match diesel::sql_query(delete_query.query).execute(&*self.conn.read().unwrap()) {
            Ok(data) => Ok(Response::new(ResponseChange {
                ok: true,
                desc: None,
                rows: data as _,
            })),
            Err(e) => Err(Status::not_found(format!(
                "Invalid query or Not Found: {:?}",
                e
            ))),
        }
    }
}

#[test]
fn test_sql_query() {
    use diesel::prelude::*;
    use diesel::sql_types::*;

    let conn = establish_connection();
    let s = "insert into grsql (id, name, mime, created, updated, content) values (10, \"name\", \"text/plain\", 1000, 1000, ?);";
    let data = diesel::sql_query(s)
        .bind::<Binary, _>(vec![65, 66, 99, 100, 24, 97])
        .execute(&conn)
        .unwrap();
    dbg!(&data);
    let s_select = "select * from grsql where id = 10";
    let data_select = diesel::sql_query(s_select)
        .get_result::<entity::Data>(&conn)
        .unwrap();
    dbg!(&data_select);
    let s_update = "update grsql set updated = 102358 where id = 10;";
    let data_update = diesel::sql_query(s_update).execute(&conn).unwrap();
    dbg!(&data_update);
    let s_delete = "delete from grsql where id = 10;";
    let data_delete = diesel::sql_query(s_delete).execute(&conn).unwrap();
    dbg!(&data_delete);
    assert!(false);
}

fn match_for_io_error(err_status: &Status) -> Option<&std::io::Error> {
    let mut err: &(dyn std::error::Error + 'static) = err_status;

    loop {
        if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
            return Some(io_err);
        }

        // h2::Error do not expose std::io::Error with `source()`
        // https://github.com/hyperium/h2/pull/462
        /*
         *if let Some(h2_err) = err.downcast_ref::<h2::Error>() {
         *    if let Some(io_err) = h2_err.get_io() {
         *        return Some(io_err);
         *    }
         *}
         */

        err = match err.source() {
            Some(err) => err,
            None => return None,
        };
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:7749".parse()?;
    let cruder = SqliteCruder::new();

    println!("Listening on: {:?}", addr);
    Server::builder()
        .add_service(CrudServer::new(cruder))
        .serve(addr)
        .await?;

    Ok(())
}
