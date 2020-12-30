use crate::server::Server;
use bytes::Bytes;
use glib::subclass;
use glib::subclass::prelude::*;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst::{gst_debug, gst_error, gst_info, gst_log};
use gst_base::prelude::*;
use gst_base::subclass::prelude::*;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::u32;

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
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            address: DEFAULT_ADDRESS.into(),
            port: DEFAULT_PORT,
        }
    }
}

static PROPERTIES: [subclass::Property; 2] = [
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
];

#[derive(Debug)]
enum State {
    Stopped,
    Started { stream_key: String },
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
        obj.set_automatic_eos(false);
        obj.set_format(gst::Format::Bytes);
    }
}

impl ElementImpl for RtmpSvrSrc {}

impl BaseSrcImpl for RtmpSvrSrc {
    fn start(&self, src: &Self::Type) -> Result<(), gst::ErrorMessage> {
        // TODO: Here we start the server..
        Ok(())
    }

    fn stop(&self, src: &Self::Type) -> Result<(), gst::ErrorMessage> {
        gst_debug!(CAT, obj: src, "Stopping");
        // TODO: Here we stop the server
        Ok(())
    }

    fn is_seekable(&self, _src: &Self::Type) -> bool {
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

        // gst_debug!(CAT, obj: src, "End of stream");
        // Err(gst::FlowError::Eos)

        match *state {
            State::Started { .. } => {
                let chunk = Bytes::from("Mock");
                // Here we return the buffer
                let size = chunk.len();
                assert_ne!(chunk.len(), 0);

                let buffer = gst::Buffer::from_slice(chunk);

                Ok(buffer)
            }
            State::Stopped => {
                gst::element_error!(src, gst::LibraryError::Failed, ["Not started yet"]);

                return Err(gst::FlowError::Error);
            }
        }
    }
}
