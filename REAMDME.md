# TODO
## Global
- [ ] optimize log and display trait of RtmpMessage
- [ ] redefine error

## Protocol/rtmp-handshake
- [ ] complex handshake

## Protocol/rtmp-server
- [ ] redirect
- [ ] on_play_client_pause
- [ ] fmle_unpublish
- [ ] Bug fix: rtmpdump -r "rtmp://127.0.0.1:8081/live/stream?aaa=bbb", parsed stream is "stream?aaa=bbb"

## Protocol/rtmp-client
- [ ] 

## Service/rtmp-service
- [ ] replace uuid

## Service/stream/hub
- [ ] on_auido, on_video, on_metadata
- [ ] gop cache


## Transports
- [ ] read/write timeout
- [ ] stats: kbps, bytes
- [ ] set read/write buffer
- [ ] merge read/write