# Alacrity

A HTTP 1.1 load balancer written in Rust using tokio.

## Design

The goal is to build an ELB-like load balancer that works well in the dynamic VM/container environments that are starting to be more common. It is expected that one load balancer be used for one cluster. As such, there is no


   * Servers must register with the load balancer using an HTTP POST to the management IP.
      * The POST payload contains the health check information.
   * The load balancer will keep that server active in the pool as long as the health succeeds.
   * The pool is managed by Raft, allowing a cluster of redundant load balancer servers. This should allow an active/passive setup out of the box.
      * Note: The [raft-rs](https://github.com/Hoverbear/raft-rs) crate does not currently support dynamic membership.
   * Async IO is done using tokio-core (which is built on top of mio).

Credit to Hoverbear who talked through some of the design with me.

## Running Protype

   * `RUST_LOG=alacrity cargo run --bin alacrity`
   * `cargo run --bin test-server`
      * Due to an issue with cargo not allowing two `cargo run` at the same time, I do `RUST_LOG=hyper ./target/debug/test-server`
   * `curl -vvv localhost:8080`

## High Level Roadmap

We are currently working on a [0.1.0](https://github.com/hjr3/alacrity/issues?utf8=%E2%9C%93&q=is%3Aissue%20milestone%3Av0.1.0%20) release.

## Management API Design

### Adding A Server

```
POST /servers

{
   "ip": "120.0.0.1",
   "port": "8080",
   "check": {
      "type": "tcp"
   }
}
```

### Removing A Server

Note: It is more common for a server to fall out of the pool after `n` health checks fail.

```
DELETE /servers/:id
```

### Stats

```
GET /stats
```

```
{
   "client": {
      "success": 34534,
      "failed": 33,
   },
   "server": {
      "success": 33770,
      "failed": 15,
   }
}
```

#### Detailed Stats

```
GET /stats/detail
```

```
[{
   "id": "...",
   "ip": "127.0.0.1",
   "port": "8080",
   "success": 33770,
   "failed": 15,
},{
   ...
}]
```

## License

Licensed under either of
 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
