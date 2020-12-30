# GStreamer RTMP Server Plugin

GStreamer Plugin that creates a server that is capable of receiving a RTMP stream.

### Usage

Sending a test video stream:

```bash
gst-launch-1.0 videotestsrc is-live=true ! x264enc ! flvmux ! rtmpsink location='rtmp://localhost:5000/myapp/somekey live=1'
```

Receiving a rtmp video stream:

```bash
gst-launch-1.0 -v uridecodebin uri=rtmp://localhost:1935/myapp/somekey ! autovideosink
```
