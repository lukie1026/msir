# TODO
## Global
- [ ] optimize log and display trait of RtmpMessage
- [ ] redefine error

## Protocol/rtmp-handshake
- [ ] complex handshake

## Protocol/rtmp-codec
- [ ] improvement

## Protocol/rtmp-server
- [ ] redirect
- [ ] Bug fix: rtmpdump -r "rtmp://127.0.0.1:8081/live/stream?aaa=bbb", parsed stream is "stream?aaa=bbb"

## Protocol/rtmp-client
- [ ] 

## Service/rtmp-service
- [ ] replace uuid

## Service/stream/hub
- [ ] on_aggr

## Transports
- [ ] read/write timeout
- [ ] stats: kbps, bytes
- [ ] set read/write buffer
- [ ] merge read/write

## Config
- [ ] chunkIO BufStream 128KB
- [ ] Merge write 350ms

# BUG
- [ ] 150路推流，300路拉流的压测场景下，会有概率出现推流断流
- [ ] 推流因超时断开后，stream manager中没有删除该条流记录