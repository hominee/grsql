**Grsql** is a great tool to allow you set up your remote sqlite database as service and **CRUD**(create/read/update/delete) it using gRPC.

## How to Use 

this project consist of two parts: 
- server 
- client 

each part can run independently. the `server` normally run on remote/cloud side, the `client` make request to it.

use the following code run server,
```sh 
cargo build --bin server
```
then the server starts listening on localhost port `7749`.

on another side, to run client 
```sh 
cargo build --bin client 
```
It will insert the file's content `input.txt` into remote server, then read it, update its id and finally delete it.

## Development

First of all, you need to be familiar with [proto-buffer](https://developers.google.com/protocol-buffers/docs/overview), if not, go to the site and see around.

since grsql is built on crates [toic](https://github.com/hyperium/tonic), you need some pre-developing experience of [rust](https://rust-lang.org), as well as tonic. 
see these [examples](https://github.com/hyperium/tonic/tree/master/examples) to quick start.

you can modify/add/delete the `message` and `service`  in `proto/data.proto` according to your requests
then implement them according to that in `src/server.rs`


## Notes

gRPC is designed to deal with service that processes small and frequent communication, not for large file tranfer(file upload/download), if you do so, the performance is worse than `HTTP2`.
It is better that the file is less than 1M according to [this](https://ops.tips/blog/sending-files-via-grpc/), So use it with care!

you can refer these articles for more:
[Sending files via gRPC](https://ops.tips/blog/sending-files-via-grpc/)
[Upload/Download performance with gRPC ](https://github.com/grpc/grpc-dotnet/issues/1186)
[Use gRPC to share very large file](https://stackoverflow.com/questions/62470323/use-grpc-to-share-very-large-file)

## Feature to Add 

Due to the issues mentioned above, it is desired to 
  - [ ] add a http2 module to handle large file upload/download.

To make transfer more efficient, it is resonable to 
  - [ ] add a compression
