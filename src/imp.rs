use crate::connection::{Connection, ConnectionError, ReadResult};
use crate::data::{MediaType, RtmpInput};
use crate::server::{Server, ServerResult};
use glib::subclass;
use glib::subclass::prelude::*;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst::{gst_debug, gst_trace, gst_info};
use gst_base::prelude::*;
use gst_base::subclass::prelude::*;
use once_cell::sync::Lazy;
use slab::Slab;
use std::collections::HashSet;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Mutex;
use std::{thread, u32};

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "rtmpsrvsrc",
        gst::DebugColorFlags::empty(),
        Some("RTMP Server Source"),
    )
});

const DEFAULT_ADDRESS: &str = "0.0.0.0";
const DEFAULT_PORT: u32 = 5000;

#[derive(Debug, Clone)]
struct Settings {
    address: String,
    port: u32,
    stream_key: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            address: DEFAULT_ADDRESS.into(),
            port: DEFAULT_PORT,
            stream_key: None,
        }
    }
}

static PROPERTIES: [subclass::Property; 3] = [
    subclass::Property("address", |name| {
        glib::ParamSpec::string(
            name,
            "Address",
            "The address the server should listen for incoming connections",
            DEFAULT_ADDRESS.into(),
            glib::ParamFlags::READWRITE,
        )
    }),
    subclass::Property("port", |name| {
        glib::ParamSpec::uint(
            name,
            "Port",
            "The port that the server should bind to",
            1000,
            u32::MAX,
            DEFAULT_PORT,
            glib::ParamFlags::READWRITE,
        )
    }),
    subclass::Property("stream_key", |name| {
        glib::ParamSpec::string(
            name,
            "Stream Key",
            "The stream key to expect content to be published",
            DEFAULT_ADDRESS.into(),
            glib::ParamFlags::READWRITE,
        )
    }),
];

#[derive(Debug)]
enum State {
    Stopped,
    Started {
        source: Receiver<RtmpInput>,
        position: u64,
        video_caps: Option<gst::Caps>,
        audio_caps: Option<gst::Caps>,
    },
}

impl Default for State {
    fn default() -> Self {
        State::Stopped
    }
}

pub struct RtmpSvrSrc {
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

impl ObjectSubclass for RtmpSvrSrc {
    const NAME: &'static str = "RtmpSvrSrc";
    type Type = super::RtmpSrvSrc;
    type ParentType = gst_base::PushSrc;
    type Instance = gst::subclass::ElementInstanceStruct<Self>;
    type Class = subclass::simple::ClassStruct<Self>;

    glib::object_subclass!();

    fn class_init(klass: &mut Self::Class) {
        klass.set_metadata(
            "RTMP Server Source",
            "Source/Video",
            "Creates a server capable of receiving a RTMP stream",
            "Rafael Caricio <rafael@caricio.com>",
        );

        let caps = gst::Caps::new_any();
        let src_pad_template = gst::PadTemplate::new(
            "src",
            gst::PadDirection::Src,
            gst::PadPresence::Always,
            &caps,
        )
        .unwrap();
        klass.add_pad_template(src_pad_template);

        klass.install_properties(&PROPERTIES);
    }

    fn new() -> Self {
        Self {
            settings: Mutex::new(Default::default()),
            state: Mutex::new(Default::default()),
        }
    }
}

impl ObjectImpl for RtmpSvrSrc {
    fn set_property(&self, obj: &Self::Type, id: usize, value: &glib::Value) {
        let prop = &PROPERTIES[id];
        match *prop {
            subclass::Property("address", ..) => {
                let mut settings = self.settings.lock().unwrap();
                let address = value
                    .get()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| DEFAULT_ADDRESS)
                    .into();
                settings.address = address;
                gst_debug!(CAT, obj: obj, "Set address to: {}", settings.address);
            }
            subclass::Property("port", ..) => {
                let mut settings = self.settings.lock().unwrap();
                let port = value.get_some().expect("type checked upstream");
                settings.port = port;
                gst_debug!(CAT, obj: obj, "Set port to: {}", port);
            }
            _ => unimplemented!(),
        };
    }

    fn get_property(&self, obj: &Self::Type, id: usize) -> glib::Value {
        let prop = &PROPERTIES[id];
        match *prop {
            subclass::Property("address", ..) => {
                let settings = self.settings.lock().unwrap();
                settings.address.to_value()
            }
            subclass::Property("port", ..) => {
                let settings = self.settings.lock().unwrap();
                settings.port.to_value()
            }
            _ => unimplemented!(),
        }
    }

    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);
        obj.set_live(true); // this is always a live source!
        obj.set_automatic_eos(false);
        obj.set_format(gst::Format::Bytes);
    }
}

impl ElementImpl for RtmpSvrSrc {}

impl BaseSrcImpl for RtmpSvrSrc {
    fn start(&self, src: &Self::Type) -> Result<(), gst::ErrorMessage> {
        // TODO: Here we start the server..
        // TODO: consider sharing context with other gst elements
        // - Create a socket

        let (media_sender, media_receiver) = channel();
        let mut state = self.state.lock().unwrap();
        if let State::Started { .. } = *state {
            return Ok(());
        }

        let settings = self.settings.lock().unwrap();
        let address = format!("{}:{}", settings.address, settings.port);
        let listener = TcpListener::bind(&address).map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::Busy,
                ["Failed to bind to address {}: {}", address, err]
            )
        })?;

        let (connection_sender, connection_receiver) = channel();

        // TODO: Capture the join handle and use it to gracefully shutdown
        thread::spawn(|| handle_connections(media_sender, connection_receiver));
        thread::spawn(|| accept_connections(connection_sender, listener));
        *state = State::Started {
            source: media_receiver,
            position: 0,
            video_caps: None,
            audio_caps: None,
        };

        // - Create channel to receive data (metadata and media data)
        // - Create a thread that handle connections
        //    - Wait for clients to connect
        //    - Whenever a client connects and starts sending content
        // - When a Connection is active, we can "start" the RtmpSvrSrc to publish buffers of
        //   content to the stream.
        Ok(())
    }

    fn stop(&self, src: &Self::Type) -> Result<(), gst::ErrorMessage> {
        gst_debug!(CAT, obj: src, "Stopping");
        // TODO: Here we stop the server
        // - Notify the server to stop accepting connections
        // - Notify the connections the stream ended
        let mut state = self.state.lock().unwrap();
        *state = State::Stopped;
        Ok(())
    }

    fn is_seekable(&self, _src: &Self::Type) -> bool {
        // We cannot go back to previous content
        false
    }

    fn query(&self, element: &Self::Type, query: &mut gst::QueryRef) -> bool {
        use gst::QueryView;

        match query.view_mut() {
            QueryView::Scheduling(ref mut q) => {
                q.set(
                    gst::SchedulingFlags::SEQUENTIAL | gst::SchedulingFlags::BANDWIDTH_LIMITED,
                    1,
                    -1,
                    0,
                );
                q.add_scheduling_modes(&[gst::PadMode::Push]);
                true
            }
            _ => BaseSrcImplExt::parent_query(self, element, query),
        }
    }

    fn unlock(&self, _src: &Self::Type) -> Result<(), gst::ErrorMessage> {
        // TODO: Here we abort the server
        Ok(())
    }
}

impl PushSrcImpl for RtmpSvrSrc {
    fn create(&self, src: &Self::Type) -> Result<gst::Buffer, gst::FlowError> {
        let mut state = self.state.lock().unwrap();
        let (source, position, video_caps, audio_caps) = match *state {
            State::Stopped => {
                gst::element_error!(src, gst::LibraryError::Failed, ["Not started yet"]);
                return Err(gst::FlowError::Error);
            }
            State::Started {
                ref source,
                ref mut position,
                ref mut video_caps,
                ref mut audio_caps,
            } => (source, position, video_caps, audio_caps),
        };

        loop {
            let input = match source.try_recv() {
                Result::Ok(i) => i,
                Result::Err(TryRecvError::Empty) => continue, // blocks waiting for content
                Result::Err(TryRecvError::Disconnected) => {
                    return Err(gst::FlowError::Eos);
                }
            };

            match input {
                RtmpInput::Metadata(metadata) => {
                    println!("Metadata: {:?}", metadata);

                    // TODO: check for the actual format, do not just assume
                    let mut caps = gst::Caps::builder("video/x-flv");
                        //gst::Caps::builder("video/x-h264").field("stream-format", &"avc");

                    if let Some(val) = metadata.video_height {
                        caps = caps.field("height", &val);
                    }
                    if let Some(val) = metadata.video_width {
                        caps = caps.field("width", &val);
                    }
                    if let Some(val) = metadata.video_frame_rate {
                        caps = caps.field("framerate", &val);
                    }

                    *video_caps = Some(caps.build());

                    // TODO: check for the actual format, do not just assume acc
                    let mut caps = gst::Caps::builder("audio/mpeg")
                        .field("mpegversion", &4i32)
                        .field("framed", &true)
                        .field("stream-format", &"raw");

                    if let Some(val) = metadata.audio_channels {
                        caps = caps.field("channels", &val);
                    }
                    if let Some(val) = metadata.audio_sample_rate {
                        caps = caps.field("rate", &val);
                    }

                    *audio_caps = Some(caps.build());
                }
                RtmpInput::Media(media) => {
                    // TODO: Decide what to set based on content
                    if let (MediaType::Video, Some(video_caps)) =
                        (media.media_type, video_caps.as_ref())
                    {
                        gst_info!(CAT, obj: src, "Setting {:?}", video_caps);
                        src.set_caps(video_caps)
                            .map_err(|_| gst::FlowError::NotNegotiated)?;

                        // let templ = src.get_element_class().get_pad_template("video").unwrap();

                        let chunk = media.data;
                        // Here we return the buffer
                        let size = chunk.len();
                        assert_ne!(chunk.len(), 0);

                        let offset = *position;
                        *position += size as u64;

                        gst_info!(
                            CAT,
                            obj: src,
                            "Chunk of {} bytes received at offset {}",
                            chunk.len(),
                            offset
                        );

                        let mut buffer = gst::Buffer::from_slice(chunk);

                        {
                            let buffer = buffer.get_mut().unwrap();
                            buffer.set_offset(offset);
                            buffer.set_offset_end(offset + size as u64);
                        }

                        return Ok(buffer);
                    }
                    // TODO: Handle Audio content
                }
            }
        }

        gst::element_error!(src, gst::LibraryError::TooLazy, ["No content yet"]);
        return Err(gst::FlowError::Error);
    }
}

/// Accepts TCP connections
fn accept_connections(connection_sender: Sender<TcpStream>, listener: TcpListener) {
    println!("Listening for connections...");
    for stream in listener.incoming() {
        println!("New connection!");
        match connection_sender.send(stream.unwrap()) {
            Ok(_) => (),
            Err(error) => panic!("Error sending stream to connection handler: {:?}", error),
        }
    }
}

/// Handle the lifecycle of all TCP connections by sending and receiving data
fn handle_connections(media_sink: Sender<RtmpInput>, connection_receiver: Receiver<TcpStream>) {
    let mut connections = Slab::new();
    let mut connection_ids = HashSet::new();
    let mut server = Server::new(media_sink);

    loop {
        match connection_receiver.try_recv() {
            Err(TryRecvError::Disconnected) => panic!("Connection receiver closed"),
            Err(TryRecvError::Empty) => (),
            Ok(stream) => {
                let connection = Connection::new(stream);
                let id = connections.insert(connection);
                let connection = connections.get_mut(id).unwrap();
                connection.connection_id = Some(id);
                connection_ids.insert(id);

                println!("Connection {} started", id);
            }
        }

        let mut ids_to_clear = Vec::new();
        let mut packets_to_write = Vec::new();
        for connection_id in &connection_ids {
            let connection = connections.get_mut(*connection_id).unwrap();
            match connection.read() {
                Err(ConnectionError::SocketClosed) => {
                    println!("Socket closed for id {}", connection_id);
                    ids_to_clear.push(*connection_id);
                }

                Err(error) => {
                    println!(
                        "I/O error while reading connection {}: {:?}",
                        connection_id, error
                    );
                    ids_to_clear.push(*connection_id);
                }

                Ok(result) => match result {
                    ReadResult::NoBytesReceived => (),
                    ReadResult::HandshakingInProgress => (),
                    ReadResult::BytesReceived { buffer, byte_count } => {
                        let mut server_results =
                            match server.bytes_received(*connection_id, &buffer[..byte_count]) {
                                Ok(results) => results,
                                Err(error) => {
                                    println!("Input caused the following server error: {}", error);
                                    ids_to_clear.push(*connection_id);
                                    continue;
                                }
                            };

                        for result in server_results.drain(..) {
                            match result {
                                ServerResult::OutboundPacket {
                                    target_connection_id,
                                    packet,
                                } => {
                                    packets_to_write.push((target_connection_id, packet));
                                }

                                ServerResult::DisconnectConnection {
                                    connection_id: id_to_close,
                                } => {
                                    ids_to_clear.push(id_to_close);
                                }
                            }
                        }
                    }
                },
            }
        }

        for (connection_id, packet) in packets_to_write.drain(..) {
            let connection = connections.get_mut(connection_id).unwrap();
            connection.write(packet.bytes);
        }

        for closed_id in ids_to_clear {
            println!("Connection {} closed", closed_id);
            connection_ids.remove(&closed_id);
            connections.remove(closed_id);
            server.notify_connection_closed(closed_id);
        }
    }
}
