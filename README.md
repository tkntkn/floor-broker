# floor-broker

## usage

run the broker in `serial-mode`

```
cargo run -- serial config.json localhost:8080
```

run the broker in `replay-mode`

```
cargo run -- replay /Users/tyoshida/Dropbox/sensorium-pj/FloorManager/FloorManager/tmp/20220921-215112-test/1663764672050.dat localhost:8080
```

## future
replaced with existing `floor manager`, which is written based on Processing.

## technology stack

`websocket`: floor broker launch a websocket server. The destination of floor image does not have to be explicitly indicated in the code. Also, it accepts a simultaneous and multiple access from the multiple clients. However, there has a explicit disadvantage in disconnection, which needs to be apporopreately managed in the code.

`udp`: the broker also suppports the udp Tx, since the broker is implemented on Rust, not on the browser which refuse udp as it is.

v2022-09-21
